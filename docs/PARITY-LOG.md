# Parity log — Sift Launcher against the Modrinth app

Every entry is a page and state, the pair of captures it was judged from, and the
differences found by **looking at them side by side**. A page is done when its list is
empty and the pair is attached to the commit that emptied it.

The reference is the **Modrinth app v0.22.0** (`Modrinth.App_0.22.0_x64-setup.exe`),
running on the `sift-launcher` VM in a container because its Linux build needs glibc
2.43 and the VM has 2.41 (see `docs/PLAN-SIFT.md` §2).

Captures: `tools/parity/capture_x11.py` (VM, X11) and `tools/parity/capture.py`
(Windows). Both write the same report line, and `tools/parity/compose.py` pairs them.

---

## discover / idle — 2026-10-25, first pair

Captures: 1440x900 both, window rect `0,0,1440,900` both.
Report: centre and three of four corners identical (`39,41,46`); bottom-right differs
(`2,39,18` on Modrinth, `22,24,28` on ours) — that corner lands on the rail's tail in
one and empty space in the other, so it is a window-composition difference, not a
colour difference. **Needs a real measurement, not a corner sample.**

### Matches (by eye, at 1:1 scale)

| Region | What matches |
| --- | --- |
| Left rail | Icon order and glyphs: discover, search, library, folder, server, then a gap, then the settings/reload group. Same icons, same vertical order. |
| Header | Brand mark + name at left, breadcrumb, centred nothing, `No instances running` at right, then window controls. Same pieces in the same order. |
| Tabs | `Mods`, `Resource Packs`, `Data Packs`, `Shaders`, `Servers` in the same order with the same active state. |
| Result cards | Same cards, same order (Fabulously Optimized, Zombie Invade, Cobblemon Official, COBBLEVERSE), same badges, same action buttons at the same corner. |
| Right panel | Same sections: `Getting started`, `Hide already installed`, Modrinth news card. |

### Differences found (all open)

| # | Region | What differs | Suspected cause | How to confirm |
| --- | --- | --- | --- | --- |
| OBS-1 | Right panel | A stray `1 Instance / Minecraft / Modrinth` card floats over our panel, and the panel's text column is clipped at the window edge | the pointer was still over an instance chip when captured (a hover card), **or** our panel is wider than the reference's | capture again with the pointer parked in empty header space; then measure the panel's right edge |
| OBS-2 | Tabs | Our tab labels look a touch heavier and sit ~1-2 px lower; the active underline looks wider | font weight or the underline's padding | crop both tab strips at 3x and overlay |
| OBS-3 | Search field | Ours appears ~80 px narrower than the reference's | the field's flex basis against the sort/view controls | measure both fields' left/right edges in the captures |
| OBS-4 | Right panel | `Getting started` rows' vertical rhythm looks slightly tighter in ours | row gap token | measure row pitch |
| OBS-5 | Card | The card's corner radius and border tone need a pixel check | radius token | sample the same corner point on both |

### What this entry is not

It is not a verdict on any of the five. Every row is a hypothesis from one 1500 px-wide
composite; the next pass crops each region at 1:1 and measures edges rather than
eyeballing them, and OBS-1 in particular may vanish once the pointer is parked.

---

## How to add an entry

1. `python3 tools/parity/capture_x11.py --app "Modrinth App" --page <page> --out out`
2. `python3 tools/parity/capture_x11.py --app PalantirMC --page <page> --out out`
3. `python3 tools/parity/compose.py --dir out --page <page> --state idle`
4. **Open** `out/<page>-idle-compare.png`, list every difference in the table above,
   with a region, what differs, the suspected cause and how to confirm it.
5. Fix, re-capture, and delete rows that no longer reproduce. Attach the final pair.

---

## Measured reference geometry (v0.22.0, 1440x900, discover idle)

Read with `tools/parity/edges.py` from `discover-idle-modrinth.png`. These are the
numbers the port should be built against, not eyeballed ones; every row is a window
row and every value an x where a channel moved by more than 10.

| Row | Edges found | Reading |
| --- | --- | --- |
| 20 (title/header) | 182 | dense text region: brand mark at ~12-32, then the breadcrumb; not a layout boundary row |
| 105 (tab strip) | 23 | rail ends **x=64**; content column begins **x=88** (`88,89`); tab container inner edge at 96; first tab label at **196**; a container edge at **721** |
| 150 (search row) | 224 | rail ends 64; content begins **88**; text runs from 107 |

**What this already says about OBS-3.** The content column starts at x=88 in the
reference. If ours starts at the same place, then the narrower field is not the column's
left edge but its **right** edge (or the space the sort/view controls take), which is the
next thing to measure: the same rows on `discover-idle-sift.png`.

Not yet measured, and needed before the chrome is written: the content column's right
edge, the right panel's left edge and width, the tab strip's height, the rail's icon
pitch (the first two centres), and the title bar's height. They are one `edges.py` run
each; the reason they are not in this table is that the loop was still being built when
the table was written.

---

## The egui shell's first frame — 2026-10-25

The rewritten shell renders: title bar with the accent plate and `Sift Launcher`, the five
destination rail with its glyphs, the tab strip, twelve cards through fastframe's scrolling,
and the right panel's `Getting started` rows. Its structure is the reference's; what is
left is convergence, and these are the first differences seen at 1:1 against Modrinth
v0.22.0 side by side:

| # | Region | What differs |
| --- | --- | --- |
| E-1 | Header | the reference has a page header (breadcrumb `Discover modpacks` with back/forward arrows) between the title bar and the tabs; ours goes straight from bar to tabs |
| E-2 | Title bar | ours carries the mark and the name only; the reference also carries `No instances running` and the window controls |
| E-3 | Tabs | the reference's strip is inset inside the content column with its own rounded container; ours spans the column edge to edge |
| E-4 | Rail | our rail has no utility group (settings, reload, account) and no separator between the groups |
| E-5 | Right panel | ours draws three rows; the reference has three rows, a `Hide already installed` toggle and the news card, with the toggle's own switch drawn |
| E-6 | Cards | ours are placeholders: no icon, no install button, no badges, no footer stats |
| E-7 | Backgrounds | the content column looks a shade lighter than the reference's; needs a sampled comparison rather than an eye |

### Second pass: E-1 to E-5 closed, and what the 2x crops found

The chrome now matches the reference where the first pass did not, and the way it was
found is worth recording because two of the five "differences" were the wrong diagnosis:

| Was | Actually |
| --- | --- |
| E-1 "missing page header" | there is no separate header row: the back/forward arrows, the route badge and the page's name sit **on the title bar's own 40 px line**, to the right of the brand |
| E-3 "tab strip spans the column" | the strip is a rounded container of **six** pills, `Modpacks` first and drawn as the active green pill — the count was wrong, not just the inset |
| E-2, E-4, E-5 | confirmed as diagnosed: title-bar furniture, rail utility group below its rule, the panel's toggle and news card |

All five are drawn now, from constants in `crates/sift-ui/src/geometry.rs`, and the tab
labels, breadcrumb, card titles and panel headings are set in **Inter at the reference's
weights** (400/500/600/700/800 as separate families), which is what made the strip stop
looking "lighter and smaller": egui ships one proportional face, so without registering the
weights every heading renders at 400.

### Open after this pass

| # | Region | What differs |
| --- | --- | --- |
| E-6 | Cards | still placeholders: the reference's card has an icon tile, a title with a follower glyph, a two-line summary, a tag row of badges and a footer of downloads/followers/updated, with an `Install` button at the right |
| E-8 | Search row | the field, the two selects and the count are drawn but inert; the reference's field takes focus, shows a caret and a clear button, and the selects open menus |
| E-9 | Rail | the five destination glyphs render smaller and dimmer than the reference's, which are 24 px at `--color-secondary` with a 40 px plate on hover |
| E-10 | Panel | the news card's height and the row rhythm need measuring rather than the current constants |

### Two fidelity gaps in the icon set, recorded before they cost time later

`tools/gen_icons_egui.py` emits 441 glyphs / 1,734 shapes, and cannot express two thirds of
the reference's *filled* elements faithfully:

- **16 glyphs have holes or concave outlines** (`cloud`, `x`, `cog`, `spinner`,
  `radio-button`, `minecraft`, `vanilla`, `velocity`, `tags/categories/{entities,mobs,potato}`,
  `tags/loaders/{datapack,babric}`, …). epaint's `PathShape` has no fill rule and fills with a
  triangle fan from the first point, so a hole is filled solid. Fix: triangulate in the
  generator (even-odd) and emit a `Shape::Mesh`.
- **413 of 441 glyphs use `stroke-linecap="round"`**, which `PathStroke` cannot honour; the
  zero-length dot idiom was already mitigated by emitting discs, but joins still miter where
  the reference rounds them. Fix: insert small discs at joins in the generator.

Both are generator work, not app work, and both are visible at 32 px and above — which is
the size the rail and cards use.
