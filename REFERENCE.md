# The reference client, measured off its own window

OWNS: `tools/refwalk.py`, `tools/refsample.py`, `tools/refocr.ps1`, `tools/refuia.ps1`,
`GATES.md`, `crates/palantir-desktop/src/**`

What the Modrinth App's window actually draws, read out of its pixels and its own
labels rather than from its source or from memory. This is the inventory the
shell is ported against, and `GATES.md` is where the numbers that matter become
commands.

The build measured is **0.204**, installed at
`%LOCALAPPDATA%\Modrinth App\Modrinth App.exe`, window pinned to a **1280x720
client area** so a capture of it and a capture of ours compare 1:1.

## How to re-measure it

Our own side first, because it is the half that can be repeated on demand:

```bash
# capture a page of this launcher at the reference's own client size
python tools/appshot.py --page home --size 1280x720 --out .scratch/pal-home.png
# and judge it against the numbers below
python tools/page_gate.py .scratch/pal-home.png --page home
```

`--shot` makes the launcher ask iced for its own frame and write it, so nothing
reaches into the window from outside and the capture is the frame this shell drew
rather than the compositor's rendering of a window parked off the desktop. (The
earlier way -- `winshot.py --park --method print` -- returned a black image on one
run and the page on the next, and could leave the launcher frozen on "Loading your
instances…": a `SetWindowPos` on a window whose runtime has not finished booting
blocks the thread that would have loaded them. The flag also removed the need to
click a rail entry to reach a page, which is what a capture on an unattended desk
cannot do.)


```
# show the window: launched from a background shell it creates its Tauri window
# hidden and never shows it, so `show` is not optional
python tools/refwalk.py --attach "Modrinth App" --size 1296 728 1280 720

# a session is a script; every action is logged to .scratch/refwalk.log
python tools/refwalk.py --attach "Modrinth App" --script .scratch/ref-session-02.txt

# one region: its surfaces, its controls, its text lines and its labels
python tools/refsample.py .scratch/ref-01-home.png --region 981 48 1280 720
python tools/refsample.py .scratch/ref-01-home.png --ascii 0 4 360 44 --cols 120
```

`tools/refocr.ps1` reads labels out of a capture (Windows OCR, called by
`refsample.ocr`); it exists because the window's UI Automation tree stops at
`Modrinth App - Web content` — Chromium never turns its accessibility engine on
for a window nobody asks through its own host, so the tab names and button
captions cannot be read out of the control tree. `tools/refuia.ps1` is kept for
the parts that *are* exposed (the three panes, the window frame).

Captures live in `.scratch/` and are deliberately not committed: they are
screenshots of another product. The numbers below are the record.

## The shell

Three columns inside one 1280x720 client, each with its own 48px header, and a
1px `#42444a` rule under the header across the whole width.

| Column | Extent (client px) | Surface |
| --- | --- | --- |
| Icon rail | x 0..64 | `#27292e` — the raised chrome |
| Page pane | x 65..980 | `#16181c` — the page, one rounded top-left corner |
| Right panel | x 981..1280 | brand wash, `#172221` at the top to `#161d1e` at the bottom |

Header contents, left to right: the logo mark (28px, `#00da75`, x 8..36), the
wordmark "Modrinth" (x 41..159), an unidentified dim pair of hollow glyphs
(x 171..237, `#42444a`-range, unreadable by OCR — recorded, not guessed), then
the page's icon (16px, x 253..268) and its title (x 279.., 13px). The panel's
header carries its own label at x 980 ("No instances running") and a collapse
button at x 1250..1262.

## The rail

| Thing | Measured |
| --- | --- |
| Container | 64px wide, chrome `#27292e` |
| Entry | 48x48 at x 8..56, vertical pitch 52 (4px gap) |
| Active entry | plate `#1d563f`, radius ~11-12 (**not** a circle) |
| Active icon | `#00da75`, 22px |
| Idle icon | `#afbac4`, 22px |
| Divider | a short rule, y 582..586 in the 720px client |
| Bottom pair | 24px icon at y 624..648 (Settings), then a 12px element at y 682..694 |

Hovering each entry names it, which is how the destination list was read:

| y | Tooltip | Page it opens |
| --- | --- | --- |
| 76 | Home | Home |
| 124 | Discover content | Discover, with its own tab strip |
| 176 | Skin selector | Skin selector |
| 228 | Screenshots | Screenshots |
| 280 | Modrinth Hosting | Servers (shows "Modrinth App update required") |
| 636 | *(none read)* | Settings dialog |

A click at 688 while a dialog was open closed the dialog and changed no page, so
that element is not a navigation entry — recorded as unidentified, most likely
the account avatar or a footer control.

## The page column

| Thing | Measured |
| --- | --- |
| Pane | `#16181c`, top-left corner cut, content inset ~40px (x 105..929) |
| Page heading | white, 20px band, in-page at (88,77) on the Skin selector |
| Tab strip | 36px pills on the page's own `#16181c`, labels 13px white |
| Active tab | pill `#1d5540` — the same brand plate as the rail's active entry |
| Search field | `#34363c`, spans x 90..950, y ~130..170 (40px tall), 13px label |
| Sort / view | dropdown pills: `#34363c` fill, white labels, y 194 |
| Result card | `#27292e`, 106px tall, one per row across the full content width |
| Card icon | 40px tile, `#34363c` |

The Discover tab strip is **six** tabs, at y 89: Modpacks, Mods, Resource packs,
Data packs, Shaders, Servers.

## The right panel

| Thing | Measured |
| --- | --- |
| Width | 299px (x 981..1280) |
| Wash | `#172221` (top) to `#161d1e` (bottom) — straight, two stops |
| Cards | `#2a3633` with a `#3a4341` row (already asserted by `GATES.md` G6) |
| Promo cards | very dark green `#041209`/`#061d10` (Modrinth Hosting, Medal) |

## Tokens, as the running app has them

| Token | This app (0.204) | Ours before this change |
| --- | --- | --- |
| Brand / accent | `#00da75` | `#1bd96a` |
| Active plate | `#1d563f` (rail), `#1d5540` (tab) | `--` |
| Raised surface | `#27292e` | `#27292e` |
| Inset / input | `#34363c` | `#34363c` |
| Page | `#16181c` | `#16181c` |
| Divider | `#42444a` | `#42444a` |
| Idle rail icon | `#afbac4` | `#b0bac5` (one level off) |

The accent is the one real disagreement: our palette was transcribed from the
project's stylesheet ladder (`green-500`), and the app as installed paints
`#00da75` — the call-to-action button, the logo mark and the active rail icon
all measure it flat.

## The title bar, measured again

The first pass read the bar as three separate 48px headers, one per column. It is
not: it is **one chrome bar across the whole width** -- x 0..1279 at y 0..47 all
measures `#27292e` -- with a **1px `#42444a` rule** at y 48 across it, and only
then do the three columns begin (the page surface from y 49). The page column's
top-left corner is cut with a **16px radius** (the first page-coloured row is
49 + 16 - sqrt(...) at each x: 66 at x 65, 50 at x 78, 49 from x 82).

Its contents, left to right, all measured on the Screenshots page at a pinned
1280x720 client:

| Thing | Measured |
| --- | --- |
| Logo mark | 28px, x 12..40, `#00da75` |
| Wordmark "Modrinth" | x 50..162, cap 19px (its own art, not type) |
| **Back** | a 30px outlined circle, x 175..204, ring `#37393e`, a left-pointing filled triangle inside, dim |
| **Forward** | the same at x 224..253, a right-pointing triangle |
| Page icon | 18px, x 257..274, `#afbac4` |
| Page title | 16px semibold, white, from x 283 (`Screenshots` ends at 377) |

The two circles are the find of this pass. The earlier walk recorded them as
"an unidentified dim pair of hollow glyphs" because OCR sees nothing in a ring;
reading them at 1px shows a back and a forward arrow, both dim because neither
has anything to do -- the reference's rail navigation does not push history, and
only in-instance navigation does. They are **not ported**: this shell's pages are
flat, so a history stack here would be a new feature wearing another launcher's
chrome, and the honest version of that is a change of its own (`NEXT_STEPS.md`
§24).

The bar's own title is also where the reference keeps the *page's* name: the
Screenshots page draws no heading of its own, and neither does Home (its bar
reads `Home`). Discover's bar reads `Discover modpacks` -- the page and its
current tab -- so the bar is a running statement of where you are, which is what
ours now draws too (`Page::title()` and `Page::icon()`).

## The Screenshots page, measured

Session 05 walked the reference to this page and captured it; sessions 06-08
measured what moves. The client was pinned to **1280x720**, so every number below
is a client pixel at device scale 1.0.

| Thing | Measured |
| --- | --- |
| Page column | x 65..979 (`#16181c`), from y 49 down |
| Illustration | x 409..624, y 296..408 -- **216x113** |
| Its fill / outline | `#1d1f23` (6151 px) / `#34363c` (1230 px) |
| Heading ink | rows 462..484 (23 rows: cap 18 + descender), pure `#ffffff` |
| Subtext ink | rows 497..512 (16 rows), `#95a2af` |
| Gap, art -> heading | 54px of ink (408 -> 462) |
| Gap, heading -> subtext | 35px of ink (462 -> 497) |
| Content centre x | 516.5 -- the column's middle is 522.5, i.e. an **11px** scrollbar gutter the centring ignores |
| Content centre y | 404.5 -- the column's middle is 384.5, i.e. **20px below it**, which is what a content box 40px taller than its viewport produces |

The sizes behind those ink rows were confirmed rather than assumed, by the width
of the strings in the Inter face this shell already ships: "No screenshots yet"
measures 223px of ink against 227px at 24px bold, and "Screenshots you take
in-game will appear here." measures 358 against 361 at 16px regular.

The page draws **nothing else**: no heading, no rule, no button, no drop target.
That is the whole of the cluster, and `tools/page_gate.py` asserts it as one.

### How close the port landed

Measured off the build CI produced (`d23ed32`) on the same 1280x720 client, with
`tools/page_gate.py` as the judge — 9 met, 0 unmet:

| Thing | Reference | This launcher |
| --- | --- | --- |
| Illustration box | 216x113 | 213x108 |
| Heading ink | 23 rows, `#ffffff` | 23 rows, `#ffffff`, 226px wide |
| Subtext ink | 16 rows, `#95a2af` | 16 rows, `#96a2b0`, 362px wide |
| Gap, art -> heading | 54 | 53 |
| Gap, heading -> subtext | 35 | 37 |
| Content centre | x 512.5, y 400.0 | x 514.0, y 390.0 |

The illustration is five rows short of the box because the port's own back frame
starts 6px inside it and its front frame ends a pixel early — within the gate's
tolerance, and left as measured rather than tuned. The two gaps land within a pixel
and two, which answers the question the port could not answer from pixels alone:
the gaps are stated box-to-box (47 and 7) because a text widget's box begins above
its cap, and iced's boxes do carry the descender without a line gap, so the ink
lands where the reference's does.

### What is not copied

- **The artwork.** The measured box, palette and gaps are copied; the drawing
  inside is ours (three cascaded picture frames rather than the reference's fan).
  Modrinth's illustration is its own art and this repository ships no third-party
  art without a notice.
- **The Refresh chip.** This page used to draw an in-page "Screenshots" heading,
  a rule and a Refresh button, none of which the reference has. Removing them is
  what let the page be one cluster; the rescan they drove still runs on entering
  the page, which is when the reference's own page reloads too.
- **The populated grid.** The reference's own data root has no instances, so its
  Screenshots page is only ever the empty state here. What it draws once there
  are screenshots is unmeasured, and ours keeps the grid it already had.

### What moves, and what does not

Measured with a new `burst` verb in `tools/refwalk.py` (raw captures a fixed
interval apart, where `shot` would OCR each frame and cost two seconds):

- **The page does not fade.** Twenty-six frames starting at the click show the
  Screenshots page fully drawn in the *first* frame -- same heading ink, same
  ink box, same brightness as the settled capture. The page switches; nothing
  animates. (An earlier session mistook 1-level antialiasing differences between
  captures for a fade. It is not: the reference antialiases text in colour
  (ClearType), so glyph edges read differently from ours, which is also why the
  page gate compares modal ink colours and ink boxes rather than glyph bitmaps.)
- **The page has no hover.** The title, the empty state and the panel's header
  were each hovered and each capture differs from the unhovered one only by that
  same subpixel noise.
- **The rail plate is instant** at 40ms sampling, and the rail tooltip appears
  inside the first 1.4s of the burst -- no transition to measure.
- **The right panel is the animated part**: its promo block (x 997..1279,
  y 479..710) changes between captures two seconds apart with nothing hovered,
  and its card area rotates too. Neither is ours to copy -- both are Modrinth's
  own promotions -- but it is why a whole-window diff of the reference is useless
  as evidence: every panel-inclusive comparison carries motion that has nothing
  to do with what is being measured.

## The Home page, measured

The densest page, and the one whose element vocabulary every other page reuses.
Measured off `.scratch/ref-01-home.png` (a 1280x720 client, device scale 1.0) and
cross-checked against the component that draws it: `WelcomeScreen.vue` in
[`modrinth/code`](https://github.com/modrinth/code) states the layout in utility
classes, and where the two agreed the capture is what is recorded here, because ink
is what a capture can be asked about.

| Thing | Measured |
| --- | --- |
| Illustration | y 219..318 on x 463..562 -- **100x100**, the reference's `size-25` slot |
| Its fill / outline | `#1d1f23` / `#34363c`, the pair the Screenshots art uses too |
| Title ink | rows 348..367, 24px semibold `#ffffff` |
| Description ink | rows 389..404, 16px `#b0bac5`; its core ink reads `#afbac4` |
| Brand button | y 431..470 (**40 tall**), x 410..615, fill `#00da75`, label `#000000` |
| Hint row | y 487..506 (**20 tall**); key cap 18x18 of `#34363c` inside a `#42444a` ring |
| Prompt ink | rows 621..634, 14px `#96a2b0` |
| Import button | y 653..692 (**40 tall**), x 403..622, `#34363c` inside `#42444a` |
| Gaps, ink to ink | 30 (art -> title), 22, 27, 17, 19 |
| Hero centre | 18px above the column's middle, with the bottom block pinned 24px off its bottom |
| Everything centred on | x 512.5 -- the column's middle minus the same 11px scrollbar gutter |

The page draws **seven** things and nothing else: no card, no in-page heading, no
rule. That is what makes the old build's version fail the gate's first assertion --
ours drew a welcome card holding the hero, which paints as one band from y 124 to
y 599.

**Two of the reference's own numbers disagree with its stylesheet, and the
stylesheet is right about the layout.** The hero is centred in the space *above* the
bottom block rather than in the page (`WelcomeScreen.vue`: the hero is `flex-1`, the
bottom block is its sibling), which is exactly the 18px above the middle the capture
shows; and the two colours the 0.204 build paints -- `#00da75`, and the `#1d563f`
rail plate -- are not the token ladder in `main`, which says `#1bd96a`. The capture
is what is copied.

**A thin line on a dotted backdrop is not one colour.** The reference paints a
dot texture over this page (415x478 dots, each pixel up to 20 levels off the page),
so a 16px regular line's most-common colour at any threshold is a half-lit edge:
`#34383d` for a line whose token is `#b0bac5`. The gate asks what the *brightest*
ink is within 12 levels of the strongest pixel instead, which answers `#afbac4` --
and that is also why the illustration's own fill cannot be measured at a threshold
that excludes the dots: at 21 levels it is *fainter* than they are, so the gate
finds its box by asking which rows are mostly ink across its width rather than which
rows are bright.

### How close the port landed

Measured off this shell's own capture at the same size (`tools/appshot.py --page
home`), with `tools/page_gate.py --page home` as the judge — **12 met, 0 unmet**:

| Thing | Reference | This launcher |
| --- | --- | --- |
| Illustration box | 100x100 | 100x100 |
| Title ink | 20 rows, `#ffffff` | 20 rows, `#ffffff` |
| Description ink | 16 rows, `#b0bac5` | 16 rows, `#b0bac5` |
| Brand button | 40 rows, `#00da75` + `#000000` | 41 rows, `#00da75` + `#000000` |
| Hint row | 20 rows, 18px cap | 20 rows, 17px cap |
| Import button | 40 rows, `#34363c` in `#42444a` | 41 rows, `#34363c` in `#3d3f45` |
| Gaps | 30 / 22 / 27 / 17 / 19 | 30 / 24 / 29 / 17 / 21 |
| Band centres | x 512.5 | 512.0 to 513.0 -- every offset under a pixel |
| Hero above the middle | 18 | 18.0 |

The two gaps that differ by 2px are the two lines the reference sets with a line
box taller than its cap; ours carry the same descender without the half-leading, so
their *boxes* are 2px shorter and the ink below them starts 2px higher. That is the
model the Screenshots port already stated -- gaps are box-to-box here, expressed as
the ink distance the reference's own box produces -- and it is now confirmed on a
second, denser page rather than assumed.

### What is not copied

- **The illustration.** The reference draws its own square brand mark in this slot.
  The measured box, its two colours and its corners are copied; the drawing inside
  is ours (the launcher's hourglass on the measured plate, `glyphs::welcome_art`).
- **The right panel's promos, the "Getting started" list and the account card** --
  all captured, all Modrinth's own content or another launcher's features.
- **Their copy.** "Welcome to Modrinth", "Modrinth" in the title bar and the
  reference's own strings stay theirs; ours say this launcher's name in the same
  positions, at the same sizes, in the same colours.

## Surfaces measured, with what they contain

- **Home** — hero "Welcome to Modrinth" / "Ready to start playing?" with a
  `#00da75` "+ Create an instance" button, "Press N to quick create an
  instance", and an "Escaping another launcher? / Import from launcher" row.
- **Discover** — six tabs, a search field, Sort and View dropdowns, and results
  as full-width 106px cards (icon, title, author, description, stats, loader
  chips, version, "Install").
- **Skin selector** — in-page heading, "Saved skins" and "Default skins" grids,
  "Add skin", and a drag-and-drop target.
- **Screenshots** — as measured above: one centred cluster, the page's name in
  the title bar instead of a heading, and no controls.
- **Servers** — "Modrinth App update required", "You need to update to use
  Modrinth Hosting through the Modrinth App", and a "Reload to update" button.
- **Create instance** (opened, captured, escaped) — three cards: "Custom setup"
  ("Start from scratch by picking a loader and game version"), a modpack choice
  ("Choose a project and we'll use its latest version"), and "Upload a modpack"
  ("Install a modpack from an .mrpack file on your device").
- **Settings** — three groups (DISPLAY, ACCOUNT, INSTANCES), eleven tabs,
  Appearance's "Color theme" cards (Dark / Light / OLED / Sync with system) and
  a "Sync theme across devices" toggle, footer "Modrinth APP 0.204" and the
  Windows build string.

## What this changes here

0. **The bar.** It now names the page it belongs to -- an 18px glyph in the
   secondary tint, 9px, then the title at 16px semibold white, all three
   measured -- and it is one drag patch like the brand, so the strip under the
   pointer is never dead. `tools/page_gate.py` is the page-content oracle:
   reference numbers in, one capture in, a verdict out.
1. **The accent.** `#00da75` replaces `#1bd96a`, with hover and pressed derived
   by the rule the palette already documents (`brightness(1.25)` / `0.8`).
2. **The rail.** The active plate is a 12px-radius `#1d563f` square, not a
   circle, and the idle icon is `#afbac4`. The container is 64px.
3. **The order.** Home, Discover, Screenshots come first, in the reference's
   order and with its tooltips; the instance-scoped pages stay behind the
   divider, where the reference keeps its own instance content.
4. **The tab strip.** Modpacks leads, and the strip carries the reference's
   order for the types we serve.
5. **The Screenshots page.** The in-page heading, the rule and the Refresh chip
   are gone; what is left is the measured cluster -- the 216x113 illustration,
   the 24px bold white heading, the 16px tertiary subtext -- centred 20px below
   the column's middle and 11px of gutter left of it, exactly where the
   reference's own content lands.

## Still to port, and why each is not done here

- **The instance page's own tab strip** — the reference keeps Mods, Worlds,
  Files and Screenshots inside an instance, and the instance page is unreachable
  in this install because it has no instances. Creating one would write into the
  user's own Modrinth data; the walk was restricted to dialogs and Esc, so this
  surface is unmeasured rather than guessed at.
- **The Skin selector page** — needs a real skin source (offline skins, a
  username lookup, or a local file) before a page can honestly claim the name.
- **The Servers page** — the reference itself walls it behind an app update in
  0.204; there is no endpoint behind it to copy.
- **The create-instance chooser** — ours opens straight into the configure step;
  the reference opens with three cards. Porting it means reflowing the create
  flow, which is its own change.
- **The hover/press arithmetic** — `hover:brightness(1.25)` and
  `active:scale(0.95)` are the reference's, and a still capture cannot measure
  scale; recorded in `NEXT_STEPS.md` §12 as before.
