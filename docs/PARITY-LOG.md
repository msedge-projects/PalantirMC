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
