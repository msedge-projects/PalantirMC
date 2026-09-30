# Gates: PalantirMC's shell matches the reference client

OWNS: crates/palantir-desktop/src/**, tools/panel_gate.py, tools/page_gate.py,
tools/appshot.py, tools/shellcmp.py, NOTES.md

Scope: the shell's chrome, surfaces, shape and type match the Modrinth App as
measured off its own running window rather than estimated from screenshots, and
the measurement survives as a command.

The oracle is `tools/panel_gate.py` for the shell and `tools/page_gate.py` for
what a page draws inside the pane -- the port is per page now, so the pages have
their own. Every expectation below is a record of what the reference client's own
pixels measured, and each assertion is written so that the build it replaced
fails it -- which is the only thing that makes a passing run mean anything. G7 is
the gate that proves that claim rather than assuming it, and G13 is where the
page gate does the same.

A capture of this launcher comes from the launcher: `--shot` makes it ask iced for
its own frame, write it and close, so it is the frame the shell drew rather than the
compositor's rendering of an off-screen window -- and it arrives at an exact client
size instead of whatever the work area allows. `tools/appshot.py` drives that, and
it is what the page gates are run against. The shell gates (`panel_gate.py`) still
read the older `PrintWindow` captures, which are `tools/winshot.py` grabs taken
off-screen and without activating the window.

`.scratch/pal-final.png` is the capture under test for the shell gates;
`.scratch/mr-home.png` and `.scratch/pal-new.png` are that control's two inputs and
are deliberately *not* committed -- the first is a screenshot of another product, and
the second is a superseded build. G7 is therefore environment-dependent: it is
runnable here and stands as a recorded manual result elsewhere. The reference numbers G1--G6 assert against are baked into
the checker, so those gates need nothing but the capture under test.

## State of this file: the shell it judged is being replaced

The gates below were written against a shell that matched the reference's
*geometry* while keeping a different information architecture -- a flat rail with
Mods, Worlds, Logs, Settings, Accounts and About as top-level pages, and Settings
as a page rather than the reference's modal. `REFERENCE.md` records that decision
in its "Still to port" section, and it is the main reason that shell did not read
as the Modrinth App. The rewrite in
`docs/superpowers/specs/2026-09-24-modrinth-native-rewrite.md` replaces it.

What that does to this file, stated rather than implied:

* **The measurements survive.** Every number in `REFERENCE.md` and every sampled
  value here came off the reference client's own window, and they are the head
  start the rewrite is built on. The shell's rail pitch, its circular plate, the
  head rule, the page inset and the right panel's wash are re-asserted as they
  are.
* **The gate scripts survive**, because they judge a capture against recorded
  numbers rather than against a particular build.
* **The page gates are replaced, one per page**, as each page is rebuilt -- each
  new one written so the build it replaces fails it, which is the only thing that
  makes a passing run mean anything.
* **The token gates are replaced outright.** `theme_tokens.rs` was a test-only
  receipt for 189 transcribed values while the shell painted from a hand-written
  palette; the gate that replaces it is the generated module the shell actually
  paints from, with regeneration required to be byte-identical.

Everything below is the record of these gates as they were run against that shell.
It is kept because the runs are evidence, and a retired gate that keeps its ID keeps
the source comments that cite it honest. Gates are not renumbered: a retired one
gains a note in the ledger rather than a gap in the sequence.

Result: **9 met, 0 unmet, 0 abandoned** for the build at `bbebaaa`, captured into
`.scratch/pal-bbebaaa.png` (1257x707) from the exe CI built for that commit.
G1--G7 were established on the build at `84d5f4a` (`.scratch/pal-final.png`) and
still hold there and here. G9--G10 came from the reference walk recorded in
`REFERENCE.md`, which re-measured the running app (0.204) at a pinned 1280x720
client — and they *fail* on `84d5f4a`, which is the build they replace, so the
control still discriminates with them in place. G8 is met by inspection.

G11--G19 came after that, with the first page ported element by element, and are
listed in their own section below: **9 met** for the build (`d23ed32`) that port
produced, with the capture they were judged from and the command that takes it.
G20--G29 are the second page -- Home -- and are **10 met, 0 unmet** on this build,
with the three captures that show the oracle discriminating -- and they were then
re-run against the exe CI built for it (run
[35531424667](https://github.com/MSedgeMC/PalantirMC/actions/runs/35531424667),
green in all five jobs), hash-checked against the runner's own sidecar there
(`10b99cea…0be1a`, and `df42046f…` for the GNU target) and staged as
`dist/PalantirMC.exe`: `python tools/appshot.py --exe dist/ci-cb5a98f/msvc/PalantirMC.exe
--page home --out .scratch/pal-home-cb5a98f.png` writes the same numbers as the debug
build, so **12 met, 0 unmet** hold on the release build too.

Later runs of the page gates used the launcher's own capture path rather than the
window tool, which is a change to *how* the evidence is taken rather than to what is
asserted: the same 12 assertions are judged off the same 1280x720 client either way,
and the 9 Screenshots numbers recorded below were re-measured off a capture of this
build's predecessor with that path before the Home port replaced it.

- [x] G1: the panel's gutter carries the brand tint, where the build it replaced
      painted the neutral raised grey (`#27292e`, whose green sits five levels
      *below* its blue)
  CHECK: python tools/panel_gate.py .scratch/pal-final.png --only gutter
  EXPECT: panel gate passed [gutter]
  EVIDENCE: `#172423` — green-blue +1, luminance 33.2 against chrome 40.9. The
      replaced build failed here by precondition: no cell in its panel margin was
      darker than the chrome, because its panel *was* the chrome's surface.

- [x] G2: the panel's wash ramps downward across the window, the way the
      reference's does, rather than being flat
  CHECK: python tools/panel_gate.py .scratch/pal-final.png --only ramp
  EXPECT: panel gate passed [ramp]
  EVIDENCE: green 36 at the top sample, 26 at the bottom. Clear cells were found
      at 8 of 8 sampled heights.

- [x] G3: the panel's wash lands on the reference's own two measured stops,
      `#182524` at the top and `#131a1a` at the bottom, within 5 per channel
  CHECK: python tools/panel_gate.py .scratch/pal-final.png --only matches
  EXPECT: panel gate passed [matches]
  EVIDENCE: top `#172423` against the reference's `#182524`; bottom `#131a1a`
      against `#131a1a`. The bottom is exact; the top is one level off on green
      and blue, from the gradient's endpoint sitting a few rows above the first
      sampleable cell.

- [x] G4: the page pane still matches the reference, i.e. the panel's wash did
      not leak into it
  CHECK: python tools/panel_gate.py .scratch/pal-final.png --only page
  EXPECT: panel gate passed [page]
  EVIDENCE: `#16181c`, the reference's own page colour, sampled just inside the
      pane's left padding at mid-height.

- [x] G5: the page pane's top-left corner is cut away so the chrome shows
      through -- `.app-contents`'s `--radius-xl` -- rather than the pane being a
      square rectangle
  CHECK: python tools/panel_gate.py .scratch/pal-final.png --only corner
  EXPECT: panel gate passed [corner]
  EVIDENCE: corner luminance 40.9 (`#27292e`, the chrome) against the pane's
      23.9 (`#16181c`) inside it. The replaced build measured 23.9 at the corner,
      i.e. the pane painted straight through it.

- [x] G6: a card inside the panel is brand-tinted like the reference's, not the
      neutral raised grey
  CHECK: python tools/panel_gate.py .scratch/pal-final.png --only card
  EXPECT: panel gate passed [card]
  EVIDENCE: `#2a3633` over 9010 sampled pixels -- the reference's card token
      exactly, against its `#2a3633` card and `#3a4341` row.

- [x] G7: the oracle discriminates -- it passes on the reference's own capture
      and fails on the build it replaced, so a pass is a real verdict and not a
      checker that cannot fail
  CHECK: python tools/panel_gate.py .scratch/mr-home.png .scratch/pal-new.png --control
  EXPECT: oracle discriminates
  EVIDENCE: `reference exit 0 -- panel gate passed`; `candidate exit 1 -- panel
      gate FAILED (1): panel gutter is a dark brand tint`. Run again after every
      change to the checker; two revisions of this script passed the reference
      while also passing the old build, which is the failure this gate catches.

- [x] G9: the accent is the brand green the app *paints*, `#00da75`, not the
      green-500 (`#1bd96a`) its stylesheet's ladder says
  CHECK: python tools/panel_gate.py .scratch/ref-02-page-1.png --only accent
  EXPECT: panel gate passed [accent]
  EVIDENCE: `#00da75` over 267 sampled px of the reference's Discover page, and
      the same value over 1676 px of its Home page. The build it replaced
      answers `#1bd96a` (7268 px) and fails by 27 levels on green; the build at
      `bbebaaa` answers `#00da75` over 4210 px. Found by searching rather than
      sampling: the accent is the fill of whichever primary button a page draws,
      so a fixed coordinate measures one page only.

- [x] G10: the plate behind an active rail entry is `--color-brand-highlight`
      (`#1d5540`, the accent at 25% over the chrome), not an accent wash on a
      different composite
  CHECK: python tools/panel_gate.py .scratch/ref-01-home.png --only plate
  EXPECT: panel gate passed [plate]
  EVIDENCE: the reference answers `#1d563f` under the rail entry and `#1d5540`
      in its tab strip -- one level apart, on two surfaces. The build it
      replaced answers `#264237`, the 16%-over-7%-white composite it used to
      draw, and fails. The build at `bbebaaa` answers `#1e5640` over 526 px,
      one level off the reference's rail plate.

## The Screenshots page, once it was ported element by element

G11--G19 are `tools/page_gate.py`, added with the first per-page port and listed
in the order the checker runs them. They are judged against a capture of the page
open in the build under test, taken with `tools/winshot.py` and a real click on
the rail.

Result: **9 met, 0 unmet, 0 abandoned** for the build at `d23ed32`, captured at a
1280x720 client into `.scratch/ref-pal-shots-d23ed32-1280.png` and taken from the
exe CI built for that commit (`e424a9d6…7bb0`, hash-checked against the runner's
own sidecar, staged as `dist/PalantirMC-msvc-d23ed32.exe`):

    python tools/winshot.py --launch dist/PalantirMC.exe --portable --keep \
        --settle 14 --click 32,176 --out .scratch/pal-shots.png
    python tools/refwalk.py --attach PalantirMC --script .scratch/pal-shots-session-2.txt
    python tools/page_gate.py .scratch/ref-pal-shots-d23ed32-1280.png

The capture is taken with the pointer *off* the rail, because a rail tooltip is a
popup the shell paints over the page and the gate would count it as the page's own
ink. The two controls also ran: the reference's own capture
(`.scratch/ref-07-still-1.png`, must pass) and the build this replaces
(`.scratch/pal-shots-old.png`, must fail).

**Verifying this port against its own build found two faults in the checker, not
in the page, and both were fixed rather than worked around.** The first is that the
panel's left edge is not the *strongest* vertical boundary on that side: this shell
draws a scrollbar in the panel's outer band and a resize grip outside that, both
harder edges than the panel's own side, so the checker was measuring page-plus-panel
-- whose cards put ink in every row and merged the page's three bands into one. The
second is that the page column does not run to the window's bottom in this shell:
its status strip takes the last 22px, so the column now ends where the page colour
ends. Neither could have been found from the reference's capture alone, which is the
argument for running a new gate against both builds before trusting it.

- [x] G11: the title bar names the page it is showing, which is where the
      reference puts a page's name -- the Screenshots page draws no heading of
      its own
  CHECK: python tools/page_gate.py .scratch/ref-07-still-1.png --only barname
  EXPECT: page gate passed [barname]
  EVIDENCE: the reference answers `Screenshots` in its bar; the build at
      `d23ed32` answers `Screenshots` on the Screenshots page and `Home` on its
      Home page, both captured from the runner's exe, and the build it replaced
      answers nothing at all -- its bar carried only the product name and the
      version, so the page's name existed nowhere in the window.

- [x] G12: the page column is the reference's page colour, not the inset panel
      the old empty state painted behind itself
  CHECK: python tools/page_gate.py .scratch/ref-07-still-1.png --only page
  EXPECT: page gate passed [page]
  EVIDENCE: `#16181c` over 579266 px of the reference's column. The replaced
      build measures `#34363c` there -- its empty state sat on an inset card that
      filled most of the column, which the reference has no equivalent of.

- [x] G13: the column holds exactly one cluster -- the illustration and two lines
      of text -- and nothing else, which is the whole of what the reference's
      Screenshots page draws
  CHECK: python tools/page_gate.py .scratch/ref-07-still-1.png --only cluster
  EXPECT: page gate passed [cluster]
  EVIDENCE: three bands at y 292..404 (illustration), 458..480 (heading) and
      493..508 (subtext); the build at `d23ed32` answers three of its own at
      284..391, 444..466 and 481..496. The replaced build answers *one* band at
      y 70..697:
      its in-page heading, rule, Refresh chip, inset panel and empty state all
      overlap into a single run, which is the shape of a page that has chrome of
      its own rather than a page that has none.

- [x] G14: the illustration fills the reference's 216x113 box
  CHECK: python tools/page_gate.py .scratch/ref-07-still-1.png --only box
  EXPECT: page gate passed [box]
  EVIDENCE: the reference measures 216x113 exactly (x 405..620, y 292..404 after
      the capture's 4px inset); this port's own three-frame stack measures
      213x108, five rows short of the box because its back frame starts 6px in and
      its front frame ends 1px short -- inside the gate's 8px tolerance and
      recorded rather than tuned. The replaced build's empty-state glyph is a 70px
      square, which the box gate would reject as 70x70 if the cluster gate had not
      already stopped the run.

- [x] G15: the illustration is drawn in the reference's own two colours
  CHECK: python tools/page_gate.py .scratch/ref-07-still-1.png --only colours
  EXPECT: page gate passed [colours]
  EVIDENCE: `#1d1f23` over 6151 px and `#34363c` over 1230 px -- the reference's
      artwork fill and outline, which are this palette's rail and input surfaces,
      so the build at `d23ed32` answers the same two values (17161 px and 868 px)
      while following the color theme.

- [x] G16: the heading is the reference's 24px bold white
  CHECK: python tools/page_gate.py .scratch/ref-07-still-1.png --only heading
  EXPECT: page gate passed [heading]
  EVIDENCE: `#ffffff` in a 23-row ink box; the build at `d23ed32` answers the same
      23 rows of `#ffffff`, 226px wide against the reference's 223. The size behind
      it was confirmed by the string rather than assumed: "No screenshots yet"
      measures 223px of ink in the reference and 227px at 24px bold in the Inter
      face this shell already ships.

- [x] G17: the subtext is the reference's 16px tertiary
  CHECK: python tools/page_gate.py .scratch/ref-07-still-1.png --only subtext
  EXPECT: page gate passed [subtext]
  EVIDENCE: `#95a2af` in a 16-row ink box, against this palette's
      `--color-text-tertiary` `#96a2b0` -- one level apart, and the same
      confirmation by string width: 358px of ink against 361px at 16px. The build
      at `d23ed32` answers `#96a2b0` in its own 16 rows, 362px wide.

- [x] G18: the gaps between them are the reference's -- 54px of ink from the
      illustration to the heading, 35px from the heading to the subtext
  CHECK: python tools/page_gate.py .scratch/ref-07-still-1.png --only gaps
  EXPECT: page gate passed [gaps]
  EVIDENCE: 54 and 35 on the reference's own capture, and 53 and 37 on the build
      at `d23ed32` -- within a pixel of the first gap and two of the second. This
      was the assertion the port could not make without a capture: the constants
      are box-to-box (47 and 7) because a text widget's box starts above its cap,
      and iced's box model had only been checked against a *previous* capture of
      this shell (the old empty state's 12px spacing plus its two boxes' ascents
      predicted that page's 33px ink gap exactly). The capture now says the model
      holds on this page too, so the 47 and 7 need no correction.

- [x] G19: the block is centred where the reference centres it -- on the column's
      content box rather than its border box, and 20px below its middle
  CHECK: python tools/page_gate.py .scratch/ref-07-still-1.png --only centring
  EXPECT: page gate passed [centring]
  EVIDENCE: the reference measures its content centre at x 512.5 against the
      512.5 an 11px gutter predicts, and at y 400 against 398. The build at
      `d23ed32` measures x 514.0 against 513.0 and y 390.0 against 387.5, the
      half-pixel differences being its own 1px wider column and its status strip
      shortening the column the block is centred in. That 11px is the scrollbar
      band the reference's content ignores; the 20px is the bottom spacer a page
      whose content box is 40px taller than its viewport produces, which this port
      reproduces with a top padding of the same size.

### The Home page, ported element by element

G20--G29 came with the second page port, and each is a number measured off the
reference's own capture of that page (`REFERENCE.md`). The controls are on disk and
at the same client size, from the same commands:

    reference   .scratch/ref-01-home.png      12/12 pass
    ours        .scratch/pal-home-new.png     12/12 pass
    the build this port replaces
                .scratch/pal-home-oldctrl.png  fails G20: its hero lived in a card,
                                               so its column has one band where the
                                               reference's has seven

Ours is taken by the launcher itself -- `python tools/appshot.py --page home --out
.scratch/pal-home-new.png` -- because `--shot` hands iced's own frame out of the
process (`REFERENCE.md`, "How to re-measure it"). The replaced build's capture is
`tools/winshot.py --launch dist/PalantirMC.exe --client 1280x720 --portable
--settle 12 --park --method print`; that command returned a black image on some
runs and left the launcher on "Loading your instances…" on others, which is part of
why `--shot` exists.

- [x] G20: the page column holds seven bands and nothing else -- illustration,
      title, description, brand button, hint row, prompt, import button
  CHECK: python tools/page_gate.py .scratch/pal-home-new.png --page home --only clusters
  EXPECT: page gate passed [clusters]
  EVIDENCE: 7 bands at y 203..302, 332..351, 375..390, 419..459, 476..495,
      587..600, 621..661. The reference's own capture gives the same seven, 4px
      higher because its page column starts there. The build this replaces gives
      one band, y 124..599: its hero sat inside a welcome card.

- [x] G21: the illustration fills the reference's 100px slot
  CHECK: python tools/page_gate.py .scratch/pal-home-new.png --page home --only illustration
  EXPECT: page gate passed [illustration]
  EVIDENCE: 100x100 on both. The first attempt measured 55x77 -- the brand's tall
      logo fitted into a square slot leaves 45 of the 100 pixels empty, and every
      gap below it a dozen pixels longer than the reference's. The slot is drawn
      now (`glyphs::welcome_art`: the measured plate, the measured two colours,
      this launcher's own mark on it).

- [x] G22: the title is the reference's 24px semibold white
- [x] G23: the description is the reference's 16px `#b0bac5`
  CHECK: python tools/page_gate.py .scratch/pal-home-new.png --page home --only title
  EXPECT: page gate passed [title]
  EVIDENCE: 20 ink rows of `#ffffff`, and `#b0bac5` exactly on ours against
      `#afbac4` on the reference -- its WebView antialiases thin type in colour,
      so the gate asks what a line's *brightest* ink is rather than what colour is
      most common in it, which on a dotted backdrop answers a half-lit edge.

- [x] G24: the brand button is 40px of `#00da75` with black ink on it -- the pair
      `--color-brand` and `--color-accent-contrast` resolve to in dark
  CHECK: python tools/page_gate.py .scratch/pal-home-new.png --page home --only brand
  EXPECT: page gate passed [brand]
  EVIDENCE: 6441 px of `#00da75` with 161 px of `#000000` on the reference, 6534
      and 258 on ours, both 40 rows tall (ours measures 41 because the button's own
      border is a row of fill at the edge).

- [x] G25: the hint row is 20px with a 20px key cap inside a `#42444a` ring
  CHECK: python tools/page_gate.py .scratch/pal-home-new.png --page home --only hint
  EXPECT: page gate passed [hint]
  EVIDENCE: 20 rows on both, cap 18px on the reference and 17px on ours (the cap's
      own corner radius cuts its outer columns), ring `#42444a` on both.

- [x] G26: the block at the bottom is a 40px button of `#34363c` inside a `#42444a`
      ring, under a 14px `#96a2b0` prompt
  CHECK: python tools/page_gate.py .scratch/pal-home-new.png --page home --only import
  EXPECT: page gate passed [import]
  EVIDENCE: prompt `#95a2af` / `#96a2b0` and button `#34363c`, 40 rows, ringed
      `#42444a` / `#3d3f45`. This assertion is also what caught the block sitting
      against the left edge of the page -- a shrink-width child of a column whose
      other child fills is laid out at the left, 318px from the centre every other
      band shares.

- [x] G27: the hero's own rhythm, ink to ink -- 30 / 22 / 27 / 17 / 19
  CHECK: python tools/page_gate.py .scratch/pal-home-new.png --page home --only gaps
  EXPECT: page gate passed [gaps]
  EVIDENCE: 30 / 24 / 29 / 17 / 21 on ours against 30 / 22 / 27 / 17 / 19 on the
      reference. The two 2px gaps are the two lines whose reference line box is
      taller than its cap; the model the Screenshots port stated (gaps box-to-box,
      expressed as the ink distance the reference's box produces) now holds on a
      second and denser page rather than from one measurement.

- [x] G28: every band is centred where the reference centres it -- including the
      11px scrollbar gutter its content ignores
- [x] G29: the hero is centred in the space *above* the block at the bottom, 18px
      above the column's middle
  CHECK: python tools/page_gate.py .scratch/pal-home-new.png --page home --only centring
  EXPECT: page gate passed [centring]
  EVIDENCE: band centres within 1px of 512.5 on ours (offsets -0.5, -1.0, -0.5,
      -0.5, 0.0, -0.5), and the hero 18.0px above the column's middle against the
      reference's 18.

- [x] G8: the two windows agree as a picture, judged by looking at them side by
      side rather than by any number -- the acceptance the user actually asked
      for, and the one no assertion above can stand in for
  EVIDENCE: inspected `.scratch/pal-final.png` against `.scratch/mr-home.png`.
      The page reads as a rounded panel inset in the chrome, the panel is a dark
      green-tinted column, and the checklist shows a hollow ring for the
      outstanding step and green discs with checks for the two complete ones --
      the reference's own marker vocabulary. Differences that remain and are not
      claimed by any gate above are recorded in `NEXT_STEPS.md` §12.

## The Discover page, and the loading page

Two surfaces at once, because they are the two the port was waiting on: Discover's
own chrome, and the splash the launcher shows while it loads.

```
python tools/page_gate.py .scratch/ref-02-page-1.png --page discover   # the reference
python tools/page_gate.py .scratch/pal-discover.png  --page discover   # ours
```

The captures are 1280x720 clients, ours from
`python tools/appshot.py --exe target/debug/PalantirMC.exe --page browse --out .scratch/pal-discover.png --size 1280x720`,
which opens the launcher off the desktop and has it write its own frame -- no
window control, no cursor, no screen capture.

- [x] G30: the page draws **no heading** above its tab strip -- the first thing in
      the column is a plate, which is what the reference does and what the build
      this port replaces did not (it drew an in-page "Browse Modpacks" heading)
  CHECK: python tools/page_gate.py .scratch/pal-discover.png --page discover --only heading
  EXPECT: page gate passed [heading]
  EVIDENCE: the first band's fill is `#27292e`; the replaced build's first band is
      `#ffffff` ink on the page.

- [x] G31: the strip is **47 rows** of tabs with the selected one filled
      `#1d5540`, and the labels are inside it
  CHECK: python tools/page_gate.py .scratch/pal-discover.png --page discover --only strip
  EXPECT: page gate passed [strip]
  EVIDENCE: 48 rows and 545px wide on ours, 47 and 607 on the reference; the
      replaced build's was 23 rows of 28px chips with no plate at all.

- [x] G32: the search field is the raised inset colour, **48 rows**, and spans the
      column -- the reference has **no Search button** beside it
  CHECK: python tools/page_gate.py .scratch/pal-discover.png --page discover --only search
  EXPECT: page gate passed [search]
  EVIDENCE: `#34363c` in 47 rows, 879 of the column's 903 (the reference: 48, 851
      of 869); the replaced build's field was 37 rows and 452 wide, with a button
      in the other half.

- [x] G33: the two gaps are the reference's own **7px and 8px** box to box
  CHECK: python tools/page_gate.py .scratch/pal-discover.png --page discover --only gaps
  EXPECT: page gate passed [gaps]
  EVIDENCE: 7px after the strip on both clients. The replaced build was 16px, from
      ink-to-ink numbers taken off a different crop.

- [x] G34: the Sort row follows the field by **8px** and is **36 rows** tall
  CHECK: python tools/page_gate.py .scratch/pal-discover.png --page discover --only sort
  EXPECT: page gate passed [sort]
  EVIDENCE: 8px and 35 rows on ours, 8 and 36 on the reference.

- [x] G35: the title bar names the page -- `Discover modpacks`, the type in the
      bar because the page draws no heading of its own
  CHECK: python tools/page_gate.py .scratch/pal-discover.png --page discover --only barname
  EXPECT: page gate passed [barname]
  EVIDENCE: the bar reads `Discover` and `modpacks` (OCR splits the two words when
      the kerning opens a gap, which is why this page's check looks for the type
      rather than for one exact line). The replaced build read `Browse`.

The loading page's gates are unit tests rather than captures, and deliberately so:
the splash is up for 800 milliseconds, so a capture of it is a race, while every
number in it is a pure function of the clock in `app::SplashState`.

- [x] G36: the bar ramps at the reference's rate -- +2% every 5ms, ceiling 95 --
      and reads full once the work behind it is done
  CHECK: cargo test -p palantir-desktop the_loading_page_fills_at_the_references_rate
  EXPECT: test result: ok
  EVIDENCE: 40.0% at 100ms, 95.0% at 30s, 100.0% after `loaded()`.

- [x] G37: the page keeps `MIN_DISPLAY_MS` (500ms) before it fades, fades over the
      reference's 300ms ease-in-out, and never fades while its work is unfinished
  CHECK: cargo test -p palantir-desktop the_loading_page_keeps_its_minimum_display
  EXPECT: test result: ok
  EVIDENCE: opaque at 499ms, mid-fade a quarter of the way in, gone (and `done()`)
      at 800ms; a page whose scan never lands stays opaque for 30s.

- [x] G38: the splash's background is the reference's four layers resolved --
      `#1a2322` at the top of the window, `#151a1f` at 97.29% down it, opaque
  CHECK: cargo test -p palantir-desktop the_loading_pages_background_is_its_three_layers_resolved
  EXPECT: test result: ok
  EVIDENCE: the two stops match the tokens composited by hand, within 0.01, and the
      fade is alpha-only (the colours never move -- a browser's `opacity`, not a
      dim towards black).

- [x] G39: the plate behind the active rail entry is a **circle**, not a rounded
      rectangle -- the one thing the colour assertion above cannot see, and which
      this shell got wrong for a release
  CHECK: python tools/panel_gate.py .scratch/port-ours-shots4.png --only plate
  EXPECT: panel gate passed [plate]
  EVIDENCE: ours: top row 14px = 31% of the widest 44px; the reference's own Home
      capture: 14px = 33% of 42px. Renderings of the two candidates at the same
      48px read 33% for a circle and 62% for a 12px radius, so the 40% threshold
      is the shape rather than a tolerance. The replaced build drew the 12px
      radius, off a reading of the plate's corner that said 11.

- [x] G40: the plate is as wide as it is tall, which is what separates "a circle on
      a 48px entry" from "a circle on a smaller one"
  CHECK: python tools/panel_gate.py .scratch/port-ours-shots4.png --only plate
  EXPECT: panel gate passed [plate]
  EVIDENCE: ours 44px against a 46px height; the reference 42 against 46. A square
      plate passes this and fails G39; a 12px radius does the same. Only a circle
      passes both.

- [x] G41: the head's rule is at y 48, the pane's corner is a 20px radius, and the
      rail's plate rows are the reference's -- measured on **both** clients, which
      is what catches a difference between two windows that each look right alone
  CHECK: python tools/shellcmp.py .scratch/ref-01-home.png=REFERENCE
      .scratch/port-ours-shots4.png=OURS
  EXPECT: shell comparison passed
  EVIDENCE: the rule at y 48 on both; the pane's corner rows 62/53/50/49 there
      against 62/54/50/49 here, one row apart at one of four probes; the plate's
      rows (top, widest, bottom) 16/42/16 there against 14/44/14 here -- symmetric
      on both, which a rounded rectangle cannot be. Before the 6px band above the
      head was removed, this shell's rule was at y 54 and its plate began at y 51
      where the reference's begins at 48, a 6px offset in every vertical
      measurement in the window, invisible to either client alone and caught by
      this comparison.

## The interaction, on every control at once

G42--G45 are unit tests rather than captures, and they have to be: a capture is
of a control nobody is pointing at, so no picture of either client can show a
hover. What they assert instead is the reference's own rule, which its components
state in full (`REFERENCE.md`, "The interactions").

```
cargo test -p palantir-desktop --locked theme::tests
```

Result: **6 met, 0 unmet** on commit `b7cb7a8`, whose CI run
[35601175367](https://github.com/MSedgeMC/PalantirMC/actions/runs/35601175367) is
green in all five jobs, with the string checks of the first pass (`G45`'s
original form) folded into it and deleted.

- [x] G42: a hover goes the way the theme says it goes -- brighter in dark, and
      **darker in light**, which is the half this shell did not have
  CHECK: cargo test -p palantir-desktop hover_goes_the_way_the_theme_says
  EXPECT: test result: ok
  EVIDENCE: `--hover-brightness` is 1.25 in `.dark-mode` and 0.9 in
      `.light-properties`; the test switches the theme in force and asserts the
      direction of the move, not just the factor. The build this replaces answers
      `1.25` under both themes -- its `HOVER_BRIGHTNESS` was one constant with a
      comment saying light was 0.9 "not yet carried over" -- so it fails here by
      the first light-theme assertion.

- [x] G43: the hover is a filter over the whole control -- fill, label and ring --
      so a control with no fill still answers the pointer
  CHECK: cargo test -p palantir-desktop hover_moves_the_label_and_the_ring_too
  EXPECT: test result: ok
  EVIDENCE: the head's ring (a border and a label, `background: None`) brightens
      both; the `size=lg` base button moves all three of its colours by 1.25. The
      replaced build lightened the fill only, which on an outlined button is a
      hover that draws nothing.

- [x] G44: a disabled control is `opacity-50` on the whole element, and a quiet
      button does not gain a fill by being disabled
  CHECK: cargo test -p palantir-desktop disabled_buttons_are_dimmed
  EXPECT: test result: ok
  EVIDENCE: fill, label and ring all at 0.5; the ghost's `background` stays
      `None`. The replaced build faded the fill to 0.35 and the label to 0.4, and
      *added* a surface behind the ghost so the state would read.

- [x] G45: the numbers above are still the reference's -- read out of
      `vendor/modrinth-app` rather than restated in a comment, together with every
      colour this shell paints
  CHECK: cargo test -p palantir-desktop --locked reference_tokens
  EXPECT: test result: ok
  EVIDENCE: `crates/palantir-desktop/src/reference_tokens.rs` parses the
      reference's own `variables.scss` and `defaults.scss` (`--name: value;`
      blocks, `var()` chains followed, gradients read as stops) and compares the
      result with the palette, the radii, the interaction factors and the type
      scale. G46b and G46--G50 are its six tests, each listed below with what it
      is for.

## The token gate: every transcribed value, against the file it came from

G46--G50 are one module, `crates/palantir-desktop/src/reference_tokens.rs`, and
are unit tests rather than captures for the same reason G42 is: half of what they
check -- a hover factor, a press -- is not in any picture. They are the answer to
a question the port could previously only answer in a comment: *is this value
really the reference's?*

- [x] G46: every colour claimed to be a token is that token, in all three modes
  CHECK: cargo test -p palantir-desktop --locked every_transcribed_token_is_the_references
  EXPECT: test result: ok  EVIDENCE: **72 transcribed values and 24 declared deviations** across dark,
  light and OLED, each compared with the token it names; a disagreement
  prints our value, the reference's, and the file and line the token is
  declared on. The table also holds the **five radius constants**
  (`R_CARD`/`R_BUTTON`/`R_CHIP`/`R_MODAL` against `--radius-lg`/`-md`/`-sm`),
  which are modeless and live on the `html` block, so they are read out of the
  generated copy rather than out of the per-mode maps -- whose line numbers can
  only ever name one sheet. Two radii are deliberately *not* claims, and the
  reasons are the record: `R_KEYCAP` is 6 from Tailwind's own `rounded-md`
  rung, not Omorphia's 12, and `R_BUTTON_LG` is 14 from ButtonFrame.vue's
  literal `rounded-[14px]` on the `lg` size -- a per-button-size radius, not a
  rung of the ladder. A radius claim would pin either to the wrong scale. It
  reports `189 tokens, 29 held, 160 not held` with `--nocapture`, which is the
  list a page that ports a new token pulls from.
      That first number was 148 until this pass, because the report counted the
      tokens in its own three maps -- `.light-properties` and the two modes that
      override it -- while the reference's light mode is also the `html` block
      (the gaps, the radii, the ad colours, the ring) and `body` (the whole type
      ladder). It now counts every name either sheet declares, names only, so no
      line number has to claim a sheet it did not come from. That it lands on the
      same **189** the vocabulary walk below reads is two independent readers
      agreeing on the size of the vocabulary, which is the number a porting pass
      plans against.
      **The control that makes it a gate:**
      changing one palette value by one 8-bit level (`surface` `#27292e` ->
      `#26282d`) fails it with
      `palette.surface [Dark]: ours #26282d, reference #27292e` and
      `variables.scss:237  --surface-3: #27292e`, plus the alias that carries the
      same value (`--color-raised-bg`).

- [x] G46b: a tree that is not checked out skips rather than fails
  CHECK: cargo test -p palantir-desktop --locked a_tree_that_is_not_checked_out_skips_rather_than_fails
  EXPECT: test result: ok
  EVIDENCE: `UPSTREAM.md` promises that removing the vendored tree changes no
      test, and five tests are a strange way to keep that promise if the way to
      check it is to remove the tree (which is a thing that was tried, and left
      the tree moved for a while). Every reader here takes the tree's root as an
      argument for this test's sake, so the skip path is asserted with a path
      that is not there.

- [x] G47: a colour cannot be added to the palette without being classified
  CHECK: cargo test -p palantir-desktop --locked every_palette_field_is_accounted_for
  EXPECT: test result: ok
  EVIDENCE: the palette's field names are read out of `theme.rs` itself, so the
      table above cannot go stale: a new field fails this test until it is a
      token, a stop or a deviation with its reason written down. This is what
      makes G46 meaningful -- a table that can omit a value proves nothing about
      the values in it.

- [x] G48: the interaction factors are the reference's
  CHECK: cargo test -p palantir-desktop --locked the_interaction_values_are_the_references
  EXPECT: test result: ok
  EVIDENCE: `theme::hover_brightness()` against `--hover-brightness` in each of
      the three modes, `theme::DISABLED_OPACITY` against `disabled:opacity-50`,
      and the two things this shell deliberately does not draw
      (`active:scale-[0.97]`, `duration-150`) asserted to still be in
      `ButtonFrame.vue` so the reason recorded for them cannot outlive the
      reference.

- [x] G49: every text size is a rung of the reference's ladder, a `text-[Npx]` it
      writes itself, or a declared measurement
  CHECK: cargo test -p palantir-desktop --locked every_text_size_is_the_reference_or_a_measured_one
  EXPECT: test result: ok
  EVIDENCE: the ladder parsed from `defaults.scss` is asserted to still be
      `10/12/14/16/18/20/24/32/48`, and 197 `.size()` call sites across this
      crate are read out of the sources: 195 are on that ladder or among the
      sizes the reference writes itself (`text-[8px]`…`text-[13px]`), and 2 are
      the declared measurements 15 and 28 with the reason each was chosen. A new
      size fails the test until it is added to `MEASURED_SIZES` with a reason; a
      `MEASURED_SIZES` entry nothing draws any more also fails, so the list
      cannot rot.

- [x] G50: every weight this shell draws is one the reference names
  CHECK: cargo test -p palantir-desktop --locked every_font_weight_is_the_references
  EXPECT: test result: ok
  EVIDENCE: the reference's `--font-weight-*` values plus its Tailwind classes
      (which is where 600 comes from -- the token ladder has no semibold and
      `font-semibold` appears 531 times), against the five Inter faces this crate
      ships: 400, 500, 600, 700, 800. It also pins the two the port claims to
      follow: `--font-weight-text` is 500 and `--font-weight-heading` is 800.

## The vocabulary gate: the whole design system, copied and checked

The token gate above asks whether the values this shell *paints* are the
reference's. This one asks a larger question: **does the shell hold every token
the reference declares, at the value the reference declares it?** The two are
kept apart because they answer different things -- one is about the palette, the
other about the copy the rest of the port will draw from.

`tools/gen_tokens.py` reads the two sheets at the commit `UPSTREAM.md` pins and
writes `crates/palantir-desktop/src/theme_tokens.rs`.

- [x] G51: the shell's copy of the reference's vocabulary is the sheets
  CHECK: cargo test -p palantir-desktop --locked the_generated_vocabulary
  EXPECT: test result: ok
  EVIDENCE: **189 tokens in each of four modes** -- 144 colours, 20 lengths, 9
      numbers, 3 gradients and 13 kept as the reference's own text, in light,
      dark, OLED and retro -- with the reference's own cascade built from its
      `@extend` lines (`html` extends `.light-properties`; `.oled-mode` and
      `.retro-mode` extend `.dark-mode`) rather than assumed, `var()` chains
      followed to the end, `rem` resolved at the reference's 16px root, and every
      row carrying the **file and line** it is declared on. The copy has a second
      half: `SCOPED`, **139 declarations in 21 files** -- the tokens a component,
      a page or `classes.scss` sets for one selector, which have no mode to
      resolve in and no cascade to merge into: `--top-bar-height` and
      `--left-bar-width` on `.app-contents`, the `--brand-gradient-*` wiring on
      `.app-sidebar`, the `--os-*` scrollbar knobs a combobox configures,
      `--ease-out-expo` on `:root`, the medal-promotion colours in `global.scss`,
      and the per-card hover factors (`.instance-item`, `[--hover-brightness:
      1.1]`) that §27 recorded as not carried over. Those are held now, each with
      the selector that sets it.
      What makes it a copy rather than a transcription is that the sheets are read
      a *second* time, in Rust, by parsers that share no code with the generator:
      `the_generated_vocabulary_is_the_sheets` re-derives the cascade and compares
      **756 rows** key by key, naming the file and line of any disagreement;
      `the_generated_vocabulary_covers_every_declared_token` collects the declared
      set by a line scan -- a third opinion, so two readers cannot collide on one
      blind spot -- and fails if the copy is missing a token or holds one no sheet
      declares; and `the_generated_vocabulary_holds_the_tokens_the_palette_paints`
      asserts that the values the palette gate checks are *in* the copy, so the
      palette cannot stop being transcribed from the reference while both gates
      stay green. All three take the vendored root as an argument and skip if it
      is absent, which is `UPSTREAM.md`'s promise kept without moving 1,857 files.
      **The control:** changing one byte of one row (`#1bd96a40` -> `#1bd96a41`)
      fails it with six rows -- the token itself plus every alias carrying the same
      value -- each printed as
      `--color-brand-highlight [DARK]: generated ..., sheets ...` and
      `vendor/modrinth-app/assets/styles/variables.scss:355  --color-brand-highlight`.
      **What the control found first was the harness's own bugs**, all four of
      them: a `u8` written with `{}` instead of `{:02x}`, which rendered every
      `#rrggbb` in the sheets as decimal digits concatenated (`--surface-1` came
      back `#252523` where the copy says `#191917`); chain lookups keyed *with* the
      `--` the walk strips, so every `var()` hop reported a missing token instead
      of the value the chain ends on; a gradient with a stop it could not encode
      classified `Gradient` where the generator calls it `Text`; and the one real
      disagreement between the two languages -- `rgba(27, 217, 106, 0.7)` is
      178.5, and Python's `round` is half-to-even where Rust's is half-away, so
      the two readers chose 178 and 179. Both now add 0.5 and truncate, which is
      the one rule the two can state identically.
      Regenerating is `python tools/gen_tokens.py`; nothing in the file is edited
      by hand, and the gate's failure message is the instruction to re-run it.

- [x] G52: the scoped half of the copy is the sheets too
  CHECK: cargo test -p palantir-desktop --locked the_generated_scoped_vocabulary_is_the_sheets
  EXPECT: test result: ok
  EVIDENCE: the same standard as G51, applied to the 21 files outside the two
      global sheets. The tree is walked here, each file is parsed by a reader that
      shares no code with the generator -- innermost rule tracked, at-rules
      transparent, multi-line values joined -- and the two readings are compared
      keyed by `file:line`, naming the selector, the token and both values when
      they disagree. **The control:** one byte of one row
      (`cubic-bezier(0.16, 1, 0.3, 1)` -> `...2)`) fails it with
      `ui/src/styles/tailwind-utilities.css:2`, the selector and both values.
      **What it found was two bugs in the readers, which is what a second reader
      is for.** The token was spelled two ways: the generator wrote `--ease-out-expo`
      and the Rust reader stripped the dashes, so every row of the table differed
      by two characters -- reported as all 139 rows wrong rather than as one, which
      is how a spelling difference looks. And the mode-table convention had been
      copied without thinking: a line that starts with `--` *and holds no colon*
      is a continuation of the property above it, not a declaration
      (`transition: --_top-fade-height 0.05s linear,` in `ScrollablePanel.vue`),
      and reading it as one invented a token called
      `--_top-fade-height 0.05s linear,` that the copy held as if the reference had
      declared it. The comparison could not see that one -- both readers agreed on
      the invented row -- and G53's count could.

- [x] G53: the scoped copy is complete, counted by something neither parser can fool
  CHECK: cargo test -p palantir-desktop --locked the_generated_scoped_vocabulary_covers_every_declared_token
  EXPECT: test result: ok
  EVIDENCE: a declaration is a line that starts with `--` and holds a colon, which
      is a rule a line scan can apply without any of the parsers' state: every
      style file's declaration count is read that way and compared with the number
      of rows the copy holds for that file. A walker that skipped a file, lost
      track of a block, or invented a row fails here -- and this is the test that
      caught the invented `--_top-fade-height` row above, in a file where both
      parsers agreed with each other. It reports
      `ScrollablePanel.vue: the sheets declare 3 token(s) on lines of their own, and the copy holds 4`.

      Result: **5 met, 0 unmet** on commit `3b39f76` and the scoped work after it,
      whose CI runs are
      [35623738312](https://github.com/MSedgeMC/PalantirMC/actions/runs/35623738312)
      and [35624755229](https://github.com/MSedgeMC/PalantirMC/actions/runs/35624755229),
      both green in all five jobs, `Test workspace` included. Nothing here draws,
      so no page gate was re-run: this module reads files and compares strings, and
      the exe the run built is unchanged in behaviour from the one before it.

## The tween: how a hover is carried, and what it costs to start one

G42--G45 are about the *numbers* a control's pointer state is worth. This
section is about the frames between them: the reference draws every interaction
as a `transition ... duration-150` rather than as a state change, and until this
work the shell drew the right factors on one frame. Two things had to be built,
and the second one took reading iced's runtime rather than its widgets.

- [x] G54: a hover, a press and the modal are tweens on a deadline, not frames
  CHECK: cargo test -p palantir-desktop --locked anim::
  EXPECT: test result: ok
  EVIDENCE: `crates/palantir-desktop/src/anim.rs` holds `Interactions` -- one
      tween per control key, holding the factor the *filter* multiplies by (`1.0`
      at rest, `hover_brightness()` arrived, `PRESS_BRIGHTNESS` while a press is
      held) -- and `ModalAnim`, the backdrop's opacity and the dialog's seat,
      which is 200ms because that is what `NewModal.vue`'s overlay and dialog body
      both say (`transition: all 0.2s ease-out` / `ease-in-out`, `scale: 0.97`).
      Both are on the deadline pattern the switch and the page scroll already
      used: three late frames and thirty early ones finish at the same wall clock
      time. `hover_progress` inverts the brightness scale into the `0.0..=1.0` a
      control that *moves* needs, and it is what the switch's knob growth and the
      structural half of a button's hover both read.

- [x] G55: the pointer's arrival is a message, because a tween started while the
      view is built never moves
  CHECK: cargo test -p palantir-desktop --locked hover::tests
  EXPECT: test result: ok
  EVIDENCE: `crates/palantir-desktop/src/hover.rs` wraps a control and publishes a
      message when the pointer crosses its bounds -- twice per visit, never once
      per move, and never capturing an event, so the control inside keeps its own
      press, release and click behaviour. The module's doc comment is the
      measurement behind it: iced re-tracks a program's subscriptions in exactly
      one place, at the end of `iced_winit-0.12`'s `application::update` -- after
      a message batch and *before* the view runs -- so a tween started by a
      stylesheet that has just seen `Status::Hovered` has no frame subscription to
      carry it and paints its first frame forever. `MouseArea` publishes
      enter/leave but never hands events to its content, which leaves a `button`
      inside it inert; hence a wrapper of our own, which delegates first and
      reports after. The rule itself -- report once per crossing, and treat a
      cursor that has left the window as a departure -- is three unit tests, and
      the control for the whole mechanism is one byte of one factor in the copy.

- [x] G56: no control in the shell is built without its interaction key
  CHECK: cargo test -p palantir-desktop --locked every_control_is_built_with_its_key
  EXPECT: test result: ok
  EVIDENCE: a source scan of `crates/palantir-desktop/src/*.rs` -- comments and
      string bodies blanked first, so a doc comment mentioning a button is not a
      call -- that requires the innermost call around every `button(...)` to be
      `hover_button(...)`, which is the one place a control's key, its role's
      style and its pointer report are built from the same literal. **71 controls**
      pass it (65 in `app.rs`, 6 in `settings.rs`), and the scan also fails if it
      finds fewer than 60, so a parser that stopped seeing buttons cannot pass by
      finding none. **The control:** a probe file holding one bare `button(...)`
      fails the test with `gate_probe_tmp.rs:5`, and was removed after it did.

- [x] G57: the modal fades in *and out*, and a close draws the dialog that left
  CHECK: cargo test -p palantir-desktop --locked a_modal_fades
  EXPECT: test result: ok
  EVIDENCE: `ModalAnim::open()` is a separate question from the flag, which is why
      a modal told to close stays on screen while it fades -- and why the app keeps
      the dialog it just closed (`modal_leaving`) and `view_modal` draws *it* until
      the tween lands. Before this the arrival was wired and the departure was not:
      every close was the hard cut the open had stopped being, and the delete
      confirmation had no arrival at all, because that one site set `self.modal`
      without telling the tween. All thirteen sites now go through `set_modal`,
      which is also what makes the two impossible to disagree.

## Gates added with the rewrite, in the order they were written

G58 and G59 are the first two gates of the shell that replaces the one above.
They are unit tests rather than capture comparisons because there is no window to
capture yet: both judge a table, and both are written so the build they replace
fails them.

- [x] G58: the navigation is the reference's own, and every page the replaced
      shell put on its rail resolves to nothing at all
  CHECK: cargo test -p palantir-desktop --locked route::tests
  EXPECT: test result: ok
  EVIDENCE: 18 tests over `crates/palantir-desktop/src/route.rs`, which is
      `app-frontend/src/routes.js` route for route. The discrimination is
      explicit: `/mods`, `/worlds`, `/logs`, `/settings`, `/accounts`, `/about`
      and `/nonsense` are all asserted *not* to resolve, and those first six are
      the replaced shell's rail. The nesting is asserted too -- instance and
      project pages with their children, the six Discover tabs in the order
      `Browse.vue` draws them, the legacy `/mod/:id/:rest*` redirect, and the
      `?i=`/`?sid=` context that makes Discover an install-into-an-instance flow.
      The rail's own highlight rules are transcribed from `App.vue`'s
      `is-primary`/`is-subpage` predicates and include the two awkward ones: a
      browse page carrying `?i=` is marked on *Home*, and `/instance/:id` is
      marked nowhere, because none of the three buttons with a predicate tests
      for it.

- [x] G59: the shell's easing is the reference's easing, measured in the engine
      the reference ships inside
  CHECK: cargo test -p palantir-desktop --locked motion::tests
  EXPECT: test result: ok
  EVIDENCE: 13 tests over `crates/palantir-desktop/src/motion.rs`. The oracle is
      Chromium's: `tools/curve_samples.html` runs each of the reference's five
      curves as a real CSS animation, pauses it at each tenth of the way through
      and reads the computed style back, and the test asserts the same nine
      values per curve to 1e-5 (a Python cross-check of the same algorithm agreed
      to 1.3e-6, so the tolerance is f32 arithmetic rather than slack). Every
      duration is a lookup in the generated motion table and the lookup *fails*
      on a pair the reference does not declare, which is what catches the
      generator's own unit error: `tools/gen_theme.py` multiplied by 1000 on the
      millisecond branch too, so `transition: outline-color 150ms ease` was in
      the table as 150000ms across seven rows, and `--check` could not see it
      because both files agreed. Three assertions now hold the 2s bound -- in the
      tool, in the generated test, and here.

### The chrome's own gates, and the pages that replaced its placeholder

G60 to G65 are stages 2 and 3. Unlike G58 and G59 they are not one table each:
the copy is a 3846-message compilation of the reference's locale, "native" is a
claim about the dependency graph, and the pages are nineteen route shapes that
all have to draw in four themes and in every state they can be in.

- [x] G60: the interface's copy is the reference's own, compiled from its locale
  CHECK: python tools/gen_text.py --check
  EXPECT: text generation is byte-identical
  EVIDENCE: `tools/gen_text.py` compiles both of the reference's English locales
      -- `app-frontend/src/locales/en-US/index.json` (1516 leaves) and
      `ui/src/locales/en-US/index.json` (2330), which share no key -- into
      `crates/palantir-desktop/src/text_gen.rs`: 3846 messages, a `Key` enum, and
      one function per message that needs ICU. The four constructs the reference
      actually uses are compiled (a bare `{name}`, `{name, number}`,
      `{name, plural, …}`, `{name, select, …}`), and the five it does not
      (`date`, `time`, `list`, `selectordinal`, an apostrophe-quoted literal) are
      *refused with the key named* rather than approximated. 353 messages carry
      ICU: 80 plural nodes, 10 selects, 32 number nodes, 56 `#` markers. The
      generated file carries 15 tests of its own, including the one that counts
      the `#` markers in the source and in the emitted arms and fails if they
      disagree. The regeneration step is in `ci.yml`'s lint job beside the other
      two generators.

- [x] G61: this is a native program, and that is a measurement rather than a
      promise
  CHECK: cargo test -p palantir-desktop --locked --test native
  EXPECT: test result: ok. 4 passed
  EVIDENCE: `crates/palantir-desktop/tests/native.rs` fails the build on a
      browser or a JavaScript engine in the dependency graph (twelve crate names,
      checked against `Cargo.lock`), on anything in the tree declaring `js-sys`,
      `web-sys`, `wasm-bindgen`, `tauri`, `wry` or `webview2`, on a `.js`,
      `.ts`, `.vue`, `.svelte` or `.html` file under `crates/`, and on a
      `package.json` outside `vendor/`. The receipt for the resolved graph is
      `cargo tree -p palantir-desktop --locked --target x86_64-pc-windows-msvc -e
      normal`: it contains neither `js-sys` nor `web-sys`/`wasm-bindgen`. The
      gate was proved to bite before it was trusted -- an `assets/probe.html` in
      the desktop crate fails it with the path named, and removing the file makes
      it pass. `tools/curve_samples.html` is the one HTML file in the repository
      and is named in the gate as a deliberate exemption: it is the oracle for
      G59, runs in a browser, and ships in nothing.

- [x] G62: every route the table knows builds a page, and that page draws
  CHECK: cargo test -p palantir-desktop --locked pages::tests
  EXPECT: test result: ok
  EVIDENCE: 24 tests over `crates/palantir-desktop/src/pages/`, one module per
      page. `every_route_builds_a_page_and_that_page_draws` walks an address of
      every shape `route.rs` has -- 22 of them, including an instance id with a
      slash in it and a filtered content tab -- builds the page, draws it in all
      four themes, and then re-points the same page at its own address to prove
      the tab-change path keeps a page rather than rebuilding it. The gate the
      shell needs is the other direction: a route with no page would be a blank
      pane, and this is the test that would say so.

- [x] G63: a page that has not been answered says so, in all four of its states
  CHECK: cargo test -p palantir-desktop --locked page::tests
  EXPECT: test result: ok
  EVIDENCE: `page::Load` has five arms and `page::draw` turns four of them into
      pixels; the fifth, `Ready`, into the page's own body. The distinction the
      module exists for is that `Empty` and `Ready(vec![])` are different
      answers, and the test asserts all five arms draw in every theme. What the
      pages do with it is G64's evidence, not this one's.

- [x] G64: a page that cannot answer from disk says what is missing instead of
  drawing an empty list
  CHECK: cargo test -p palantir-desktop --locked store::tests
  EXPECT: test result: ok
  EVIDENCE: 11 tests over `crates/palantir-desktop/src/store.rs`. The sentence is
      asserted in the negative as well as the positive: it begins with the thing
      that is missing, it ends "is not implemented yet.", and it contains
      neither the word "stage" nor the word "error" -- a stage number is a word
      for developers and the reader of this sentence is not one. `a_store_with_no_instances_says_empty_rather_than_nothing_happened`
      is the other half -- an instance list that is genuinely empty is `Empty`
      and an id that is not there is `Empty` too, because "you have none yet" and
      "something broke" are the two answers a reader has to be able to tell
      apart. The readers underneath are exercised on a scratch directory: worlds
      are folders in `saves/` and a world only counts as *played* once there is a
      `level.dat`, screenshots are images and newest-name-first, a log is read
      from its end, and a byte count uses the reference's own `KiB`/`MiB` labels.

- [x] G65: the widgets the pages are made of are the reference's own rules
  CHECK: cargo test -p palantir-desktop --locked ui::tests
  EXPECT: test result: ok
  EVIDENCE: 3 tests over `crates/palantir-desktop/src/ui.rs`. Every control is
      built in all four themes, because a widget that read a token the theme does
      not declare would come back as the table's fallback and a widget that builds
      nowhere is a page that panics on someone else's machine. Two assertions are
      about the reference's values rather than a shape this file chose: a
      `colored` button's label is legible on its accent in every theme, and -- the
      one that was wrong first -- `--surface-3` is `--color-bg-raised` in *all
      four* themes, so a card is told apart from the page it sits on and not from
      the bar above it. The gate that found that is `the_card_is_the_reference_s_own_rule`,
      which had asserted the two were different because that reads like a
      difference. The reference does not make one.

### The engine, and what a cache is for

G66 to G68 are the first half of stage 4. Unlike the page gates they are not
about what a person sees: the engine is the layer whose whole job is to be
*right* about things nobody can watch — how many requests are in flight, what
happens to a half-written file when a window closes, whether "still current" was
the service's answer or the cache's assumption. So the gates are statements about
behaviour under conditions a test can produce and a user cannot: a server that
fails twice and then answers, a server that ignores a `Range`, a token cancelled
two chunks into a body.

- [x] G66: every download reports exactly once, and the queue can be stopped one
      job at a time or all at once
  CHECK: cargo test -p palantir-net --locked --lib engine::schedule
  EXPECT: test result: ok. 7 passed
  EVIDENCE: 7 tests over `crates/palantir-net/src/engine/schedule.rs`. The
      promise a progress list is a fold over: one `Started` and exactly one of
      `Finished`/`Failed`/`Cancelled` per job, asserted per id rather than by
      counting, and `Idle` once per quiet moment — including a second batch
      submitted after the first `Idle`, which is how a modpack installs its own
      dependencies. `Scheduler::cancel` on a job that is still *in the queue* is
      the case that decides whether cancelling a thousand files takes as long as
      the one in flight, and it is asserted against a queue made provable by a
      slow route: one worker, held, so the second job is certainly not started.
      `Scheduler::shutdown` cancels first and joins after, so the eight jobs in
      that test report as cancelled without a request — one of them is what a
      page folding events into a list depends on, because an entry that never
      ends is a window that will not close.

- [x] G67: a metadata answer is believed for a TTL, then revalidated rather than
      downloaded again
  CHECK: cargo test -p palantir-net --locked --lib engine::cache
  EXPECT: test result: ok. 13 passed
  EVIDENCE: 13 tests over `crates/palantir-net/src/engine/cache.rs`. The three
      paths are each held: inside the TTL the answer comes off the disk and the
      `Fetch` is never called (a second lookup with an empty map would be a 404
      if a request were made), past the TTL the validator goes back as
      `If-None-Match` and a `304` moves the stamp without moving a byte — which is
      asserted by a request count of 2 and the header on the second one — and past
      the TTL with *no* validator the body comes down again, which is the honest
      failure of a service that sends no `ETag` rather than a cache that would
      serve last week's list forever. Four rules that only bite under a condition
      are tested as such: a `500` leaves yesterday's copy and its age exactly where
      they were, a `503` twice is waited out (the waits are recorded, not slept:
      250 ms then 500 ms, which is `Backoff`'s own arithmetic and not a second
      policy), a cancelled lookup stores nothing and is not retried, and a `304`
      with nothing stored is a failure rather than an empty file nobody can ever
      correct. The body file is asserted to be the bytes the service sent — no
      envelope — because the metadata layers above this one read the cache
      directory directly.

- [x] G68: the revalidation is the service's behaviour, not the double's
  CHECK: cargo test -p palantir-net --test live --locked -- --ignored --test-threads=1 the_metadata_cache_revalidates
  EXPECT: test result: ok. 1 passed
  EVIDENCE: the first of the two live tests that have run in this session, and it
      passed against the real host: `revalidated with "6ab45c1a-6bc5e", no body sent`.
      `meta.prismlauncher.org` sends an `ETag` on
      `/v1/net.minecraft/index.json` and answers `304 Not Modified` to it, so the
      expensive half of the cache is real rather than hoped for — a version list
      that has not moved costs a header exchange instead of the document. The
      test does not *demand* a `304`, because whether a host sends a validator is
      not something the engine may assume, and it says which of the two happened;
      what it insists on is the same document either way, and that a second cache
      over the same directory — a launcher restart — can read the entry the first
      one wrote. That is the round trip the unit tests can only simulate.

- [x] G69: a file is named by its own digest, and a download only asks for what
      is missing
  CHECK: cargo test -p palantir-net --locked --lib engine::content
  EXPECT: test result: ok. 12 passed
  EVIDENCE: 12 tests over `crates/palantir-net/src/engine/content.rs`. The three
      digests the services actually publish are all carried and all computed --
      Mojang's `sha1` per asset and library, Modrinth's `sha1`/`sha512` per file,
      Prism's `sha256` -- and their hashes are asserted against published vectors
      (the empty string's `sha1`, `abc`'s `sha1`/`sha256`/`sha512`) rather than
      against themselves. A 32-character digest is *refused* rather than guessed
      at, because 32 characters is MD5 and a store that guessed would hide a
      caller's bug behind a file that verifies as something else. The two rules
      the store's name rests on are asserted: `put` refuses bytes that are not
      what they are called (and writes nothing), and `adopt` refuses a file that
      does not verify while leaving it where it was, so a caller can still resume
      it. The join with the downloader is the part that makes it a feature rather
      than a directory: `fetch` asks for nothing when the digest is already
      stored (a second call is `AlreadyThere` with a request count that did not
      move), a transfer that fails its digest is deleted rather than filed, and
      an interrupted one leaves a `.part` under the digest's own name which the
      next call resumes from -- 1024 bytes on disk after a cancellation, and
      3072 bytes over the wire when the run after it finishes the file.

- [x] G70: the publish this store believes is the publish Mojang serves
  CHECK: cargo test -p palantir-net --test live --locked -- --ignored --test-threads=1 an_asset_object_is_fetched_once
  EXPECT: test result: ok. 1 passed
  EVIDENCE: the second live test to pass in this session, and the one that says
      the store is not merely self-consistent. A real object from the live asset
      index is read out of the index by its published name, `Digest::parse` reads
      that name as a `sha1` because that is what it is, `ContentStore::fetch`
      brings the bytes through the real client, verifies them against Mojang's
      own digest, files them, and answers a second call from the disk without a
      request. A fixture would have agreed with the code on all of it; the CDN is
      the only thing that can disagree.

- [x] G71: the launcher reads Mojang's own metadata, and checks every version
      file against the digest the manifest published
  CHECK: cargo test -p palantir-net --locked --lib engine::piston
  EXPECT: test result: ok. 7 passed
  EVIDENCE: 7 tests over `crates/palantir-net/src/engine/piston.rs`. This is the
      source the launcher has never used: the shell it replaces reads
      `meta.prismlauncher.org`, which is Prism's *mirror* of piston, rewritten
      into Prism's shape. A mirror is wrong here for two reasons and both are
      gated -- every field it drops is a field this launcher would have to guess
      at, and a mirror that is a day behind is a launcher that does not know a
      version was released. The manifest is parsed tolerantly in the two places
      that tolerance is honest (an entry with no id or no URL is skipped; an
      unreadable `sha1` leaves the version without a check rather than dropping
      it) and strictly in the one place it is not: a manifest with no `latest`
      pair cannot answer the first question it is asked, so it is refused. The
      check is the part worth naming: the manifest publishes a `sha1` per version
      file, `version()` verifies the body against it, and a mismatch *forgets the
      cache entry* as well as reporting it -- a bad body left in the cache would
      be served again on the next call, and this is a file that decides the
      classpath. Both TTLs are shown working: three calls cost one request for
      the list and one for the file.

- [x] G72: the shape the launcher parses is the shape Mojang publishes
  CHECK: cargo test -p palantir-net --test live --locked -- --ignored --test-threads=1 the_live_piston_manifest
  EXPECT: test result: ok. 1 passed
  EVIDENCE: the third live test to pass in this session, and the one that says
      the source is real rather than agreed with: the live manifest is fetched,
      it names a latest release and lists it, the release's published `sha1` is
      the digest of the version file that arrives, and that file parses into a
      main class of `net.minecraft.client.main.Main`, more than twenty libraries,
      a downloadable asset index with a digest, and *no* `order` key -- which is
      the field Prism adds and the reason a mirror and the thing it mirrors are
      not interchangeable. Three real requests, and the last one is the document
      a classpath comes from.

- [x] G73: Modrinth's API is read through the engine's own cache and ceiling
  CHECK: cargo test -p palantir-net --locked --lib engine::modrinth
  EXPECT: test result: ok. 7 passed
  EVIDENCE: 7 tests over `crates/palantir-net/src/engine/modrinth.rs`, plus a
      new test over the URL builders it calls. The split it finishes is the one
      the crate had half of: `modrinth.rs` is the vocabulary (URL shapes and the
      types a response deserializes into) and this is the part that asks, with
      the engine's cache, pool and retry policy instead of a `reqwest` client of
      its own. Two TTLs, because a search and a project age differently -- five
      minutes for a search, which is a question about what people are using
      *now* rather than a document, and the metadata default for a version list,
      which changes when an author publishes -- and both are asserted, including
      that the two handles are over one directory. The cache key is the URL and
      the tests treat it that way: the same search twice costs one request, a
      different query costs two, and *the same query sorted differently* costs
      three, because a cache that ignored the sort would answer "most downloads"
      for "newest". The search URL builders became one function with the optional
      parts spelled out and three thin wrappers over it, so the facet JSON is
      encoded in one place; the wrappers' URLs are asserted byte for byte
      against what they produced before, which is what makes the refactor
      checkable rather than hopeful.

- [x] G74: the facets parameter is the JSON Modrinth documents, and the cache is
      invisible from the far side
  CHECK: cargo test -p palantir-net --test live --locked -- --ignored --test-threads=1 the_live_modrinth_api
  EXPECT: test result: ok. 1 passed
  EVIDENCE: the fourth live test to pass in this session. Modrinth takes `facets`
      as *JSON in a query string* (`[["project_type:mod"]]`, percent-encoded) and
      a client that gets it wrong receives either a 400 or a 200 full of the
      wrong kind of project; the unit tests hold the encoder against an expected
      URL, which is exactly the agreement a fixture and its code are capable of
      getting wrong together, so the live test asks the service and requires hits
      back. The same run then fetches the project the first hit names and
      requires versions with a primary file that publishes a digest -- the two
      calls the Discover page will make, against the real API, with the real
      `User-Agent`. And it asserts the cache the way a cache has to be asserted:
      the second identical search returns the same response, field for field.

### The seam, and the page that is now a client of the engine

G75 is the one gate of the wire between stage 4 and stage 3. The engine above is
right about requests and knows nothing about pages; the pages draw and ask nobody.
A seam can be wrong in three places -- the question, the answer, and the round the
answer belongs to -- so it is asserted at all three rather than at the join.

- [x] G75: the pages can ask the engine, and the answer comes back as a message
  CHECK: cargo test -p palantir-desktop --locked --bin PalantirMC
  EXPECT: test result: ok. 563 passed
  EVIDENCE: 563 tests over the desktop binary, nine of them new with this seam.
      In `store.rs` (11 tests) a search through a `MapFetch` comes back as the
      hit's own fields -- the stable id rather than the slug, the title, the
      author, the counts -- a second identical search costs no request, a failing
      answer is a reason rather than an empty list, and a store with no engine
      answers with the not-implemented sentence instead of `Ok(vec![])`.
      `a_request_runs_off_the_thread_that_draws_and_comes_back_through_a_channel`
      asserts the thread that answers is not the thread that asked, and that a
      dropped sender is an error rather than a hang. In `pages/discover.rs` (11
      tests) the controls and the request are the same thing read twice, an
      answer lands only on the round that asked for it, and a tab change puts the
      page back to *unasked* rather than to `Empty`, which is the difference
      between "not asked yet" and "the index has nothing". In `pages/mod.rs` (6
      tests) a message addressed to a page that is not on screen is dropped,
      opening a card is reported upward rather than applied by the page, and a
      search goes up to the shell and comes back down as an ordinary message.
      `palantir-net` grew the two fields a card needs and cannot invent --
      `versions` and `categories` -- and both are asserted in the API shape (24
      tests over `modrinth.rs`) and through the engine's own search (7), because
      a field dropped at the deserializer is a fact no page can recover from.

### The interaction, on the pages' own controls, and the pane that takes a theme

G76 and G77 are the two stage-3 gaps that were named as gaps: the pages' controls
changed colour on a frame boundary while the rail's plate tweened, and Settings was
a modal with a sentence in it. Both are about a decision the toolkit had already
made and the interface had not taken up yet.

- [x] G76: the pages' controls arrive at their hover over the reference's own
      150 ms, on the frames the shell asks for
  CHECK: cargo test -p palantir-desktop --locked --bin PalantirMC
  EXPECT: test result: ok. 572 passed
  EVIDENCE: 572 tests over the desktop binary, eight of them new. The crossing
      has to *be* a message rather than a read of the pointer during the view:
      iced re-tracks subscriptions after a message batch and before the view
      runs, so a tween started from the view is one frame too late for the frames
      that would carry it -- `hover.rs` has said so since the old shell needed it,
      and this is the pages taking it up. Each page's message family carries a
      `Hover { key, over, hover }`, its `update` records the crossing into the
      process-wide interaction clock, and `ui::interaction` reads the factor back
      at draw time. The clock now keeps what the pointer last reported about a
      key (`Interactions::drawn`), which is what lets a page be rebuilt every
      frame without carrying a map of its own, and `forget_pointer` is what a
      navigation calls so a control the page being left had lit is not drawn on
      the one that arrives. `Shell::animating` asks the interaction clock as well
      as the rail's plates and `Tick` advances both, which is the join asserted
      by `a_page_control_s_hover_is_a_frame_subscription_the_shell_owes`. Cards
      are the same mechanism with a name derived from what they draw
      (`ui::scoped`) and the reference's scoped brightnesses -- 0.9 for a
      project's card in every theme, 1.1 for an instance's -- because a card that
      used the theme's global hover would be the one card in the list that goes
      the wrong way. The gate that keeps the whole thing from being half-wired is
      `every_control_a_page_draws_carries_its_own_key`: it reads the eight page
      modules back (`include_str!`, so it judges the tree that was compiled) and
      refuses a call site whose second argument is still the label, which is
      exactly the shape a control without a tween has.

- [x] G77: Settings offers the reference's own colour themes, and the one that is
      taken is the one the window and the next launch use
  CHECK: cargo test -p palantir-desktop --locked shell::tests
  EXPECT: test result: ok
  EVIDENCE: the modal that has been a placeholder since stage 2 now draws
      `AppearanceSettings.vue`'s list. The shell keeps the setting rather than
      being handed a resolved theme, because the choice is made inside the window
      it changes: taking one rewrites the theme it paints from, the preferences it
      is drawn from, and the file -- and a test has no home, which is asserted, so
      a test cannot write to anyone's real settings. The list is
      `ColorTheme::options(false, current)`, which is the only owner of the
      reference's rule that retro is behind dev mode until it is the theme already
      in force; this launcher has no dev mode, so the rule is read with `false`
      and said so where it is read rather than copied as a second filter. The
      options are built by `crate::ui`, so the modal's buttons tween like the
      pages' do -- which is what the same clock is for.

### The first write, and where a created instance goes

G78 is stage 5's first slice and the first thing the interface makes rather than
asks for. Everything before it answered a question; a create makes a folder, a
config and a version profile, which is why the gate is about what happens when it
*cannot* as much as when it can.

- [x] G78: an instance can be created from the library, for the version Mojang
      says is current, and the reader lands in it
  CHECK: cargo test -p palantir-desktop --locked --bin PalantirMC
  EXPECT: test result: ok. 579 passed
  EVIDENCE: 579 tests over the desktop binary, five of them new. The write side of
      `store.rs` is asserted where it can be wrong: `create_instance` with no
      version asks Mojang for one -- the store's own fetch seam answers with a
      manifest, and the test asserts exactly one request and that the instance
      exists in the launcher's own format afterwards -- then `reload` reads the
      list again, because an instance that was just written is on disk and not on
      a page drawn from a list read at startup. A store with no launcher behind it
      answers with the sentence rather than creating somewhere invented. The
      shell's half is three claims: the rail's `+` and the library's own button
      both open the dialog (the button by *reporting* `Ask::Create` out of
      `Screen::update`, so a page never learns where instances live), a failed
      create keeps the dialog and puts the reason in it rather than closing on a
      button that did nothing, and a successful one reads the list again and
      navigates to the instance it made. `ui::button_or` is the control the dialog
      needs while its request is in flight: the same button with its press removed
      and its ink faded, rather than a second click that counts twice.

### What the other launchers are holding

G79 is the second half of the same flow. The importers have been in
`instances.rs` since before the rewrite -- the roots of every launcher this one
can read, a scan of each, and a copy that makes the id unique -- and nothing in
the shell called them: the button that imports answered with the sentence while
the code behind it sat finished.

- [x] G79: the import step lists what this machine holds, brings one in, and says
      when it holds nothing
  CHECK: cargo test -p palantir-desktop --locked --bin PalantirMC -- shell::
  EXPECT: test result: ok
  EVIDENCE: two tests, and both are about the shapes the dialog would otherwise
      get wrong. A shell with no launcher behind it -- which is what a test has --
      opens the step on an empty scan, and what it draws is the reference's own
      "no instances found" copy rather than a blank box: a dialog with a title and
      nothing under it reads as broken, which is the same failure the whole
      `page::Load` scaffold exists to prevent. The other test drives the answer the
      way the runtime would: an import that fails keeps the dialog up with the
      reason in it, and one that succeeds closes it, reads the list again and leaves
      the reader in the instance that was brought in. The scan runs when the step
      opens rather than when it is drawn, which is why the list is a field of the
      shell: it walks every launcher root, and a walk per frame is a walk per frame.

### What the instance is for

G80 is the picker the create dialog was missing. The dialog could make an
instance for Mojang's current release and nothing else, and the reference's own
flow asks: `CustomSetupStage.vue` draws a searchable combobox over Mojang's
version list with a footer that adds the snapshots and old builds to it
(`Combobox.vue`: `filteredOptions`, `DEFAULT_MAX_HEIGHT = 300`, and the
`dropdown-footer` slot whose button reads *Show all versions* / *Hide
snapshots*), opens on the version in force, and creates for whichever one is
chosen. The list is a request rather than a field read, so it is the third of the
dialog's own requests -- after a create and an import -- and it travels the same
way: a flag out of `act`, a thread, and a message back.

- [x] G80: the create dialog offers every version Mojang publishes, opens on the
      current release, and creates for the version it is on
  CHECK: cargo test -p palantir-desktop --locked --bin PalantirMC
  EXPECT: test result: ok. 584 passed
  EVIDENCE: 584 tests over the desktop binary, five of them new. The store's half
      is one request for two facts (`Store::versions`): the test asserts that the
      list comes back in the manifest's own order with the flags right -- a
      snapshot and an old beta are `release: false`, the release between them is
      not -- and that the create flow's own fallback (`current_release`) is the
      cached copy of the same document rather than a second request, which is what
      keeps the picker and the create from being told about two different moments.
      A store with no engine answers with the sentence instead of a list. The
      shell's half is four claims. Opening the dialog is a request: the test calls
      `act` directly to assert the flag is set before `handle` spends it, and that
      the picker is `Loading` while it waits. The answer is Mojang's: `Ready`
      once it arrives, and `chosen_version()` is `latest.release` -- not the first
      row of the list, which is a snapshot in the fixture. The filter is the
      reference's: releases until the footer, everything after it, a `contains`
      search over the ids (`21.3` finds `1.21.3`) and, when a search finds
      nothing, the same *No versions available* sentence a list with nothing in it
      draws -- asserted by rendering both states, because a picker that draws an
      empty box is the failure the sentence exists to prevent. And a list that did
      not arrive is a sentence and not an empty picker: `Load::Failed`, no chosen
      version, and a create that still leaves -- with nothing chosen, which is the
      store's own question to Mojang, the same answer the picker would have opened
      on. Two numbers come from the reference and are named here rather than
      measured: the options stop at 300px and scroll past it (`DEFAULT_MAX_HEIGHT`,
      which bounds the options and not the footer), and an option's own hover end
      is 1.15 (`hover:brightness-[115%]`), which is why its crossing is published
      through `Hovered::hover_with` rather than through the shell's global
      `hover_brightness`. One deviation is deliberate and is the modal layer's own:
      the reference teleports its dropdown to the window's edge and floats it over
      the dialog, and iced 0.12 has no z-order, so the same body is drawn under the
      field it belongs to -- and the chosen version is printed in the picker's own
      heading, which is where the reference's trigger mirrors it.

### The launch

G81 is the thing the plan named as the gate for retiring the old shell. `launch.rs`
is the oldest finished part of this launcher -- the worker resolves the version
through the engine, installs what is missing, extracts the natives, signs in as
the selected account (renewing a Microsoft session when it has to), spawns Java
with the argument vector from `palantir-core`, streams the game's output back and
records the play time -- and nothing in the new shell called it: the instance
page's Play answered with the not-implemented sentence while the code behind it
sat finished. Two things had to move for the new shell to be able to call it, and
both are the kind of change that is only worth making once:

* **The worker speaks facts rather than one shell's messages.** It used to send
  `crate::app::Message` values, which is the *old* shell's enum; it now sends
  [`launch::LaunchEvent`] -- a batch of lines, a level, the game coming up, the
  run ending, a session renewed -- and each shell translates. `app.rs`'s
  translation is one `From` impl and a pump; `shell.rs`'s is a variant and the
  same pump. That is what let the second shell watch a launch without the first
  one's message type leaking into it.
* **A page cannot start a process.** The instance page reports `Ask::Play(id)`
  and `Ask::Stop(id)` the way it reports a navigation, and the shell is the one
  that has the data root, the account file and the settings a launch is for. The
  pages learn what is happening through the store, which is the shape the
  reference uses too: its process list is a query keyed by instance
  (`instanceKeys.processes(id)`), not a property of the page component.

- [x] G81: the instance page's Play starts a launch in the new shell, its header
      follows the run, and Stop takes it down
  CHECK: cargo test -p palantir-desktop --locked --bin PalantirMC
  EXPECT: test result: ok. 592 passed
  EVIDENCE: 592 tests over the desktop binary, eight of them new, and the claims
      are about the seams rather than about a game having run on this machine
      (which no test here can do: a launch needs a JVM, an account and a real
      version to install). The store's half is four states and one line:
      `LaunchState` is `Idle`/`Starting`/`Running`/`Stopping` because the
      reference's header draws four different things in one place
      (`page-header/index.vue`: *Play*, *Starting...*, a red *Stop*, *Stopping...*),
      and `launch_state(id)` answers `Idle` for every instance that is not the one
      running, so a page never compares anything itself. The line keeps naming its
      instance after the run ended, which is what makes a reader who navigated away
      and back see what happened. The page's half is that Play and Stop are
      *reported* (`Some(Ask::Play("atm10"))`) rather than performed, that the four
      labels are the reference's own four, and that every state draws in every
      theme. The shell's half is the whole mechanism: `act` on an `Ask::Play`
      builds an `ActiveRunData` from the launcher's own files (data root, selected
      account -- the offline session when there is none -- and the memory and Java
      the settings name), the store says `Starting`, and the subscription that
      spawns the worker reads it. A `Progress` gives the header its level
      (`libraries 3/12 (25%)`), a `Log` gives it the last line, `Started` changes
      the *control* and deliberately not the line, and `Done` clears the run,
      records the note and reads the instance list again -- the worker writes play
      time into the instance's own files, so the library the pages are drawn from
      is stale until it is read again. Every arm checks the run id first, which is
      asserted the only way it can be: a `Done` and a `Progress` from run 1 arrive
      while run 2 is going, and the run, its state and its line are all untouched.
      A renewal is written back to `accounts.json` and the test asserts the file
      rather than the struct, because a store that was updated and never saved is
      the same as one that was never updated. Two smaller things came with it: the
      Settings modal now shows a warning when the accounts file could not be read
      (`AccountsStore::load_with_report`'s sentence, which the old shell had
      nowhere to put), and the Logs tab reads the launcher's own `logs/launcher.log`
      when the game never wrote `latest.log` -- which is exactly the log a failed
      launch leaves behind. Three deviations are deliberate and worth naming: the
      run's own line is drawn in the instance header rather than in the
      reference's bottom action bar (that surface is not built yet, and the fact is
      the same one), the *Stop* button has no stop-circle glyph (this kit's buttons
      are label-only), and the launch is not exercised end to end by any gate here
      -- a real run needs a JVM and a version to install, which is what
      `tools/launch_check.py` and the live tests exist for.

### The switch

G82 is the decision the whole rewrite was arranged around, and it is one line of
`main.rs`. The plan has said from stage 2 that the old chrome stays what runs
until the new shell can launch an instance; it can now (G81), so a plain run gets
the new shell and the old one asks for itself with `--classic`. What the flag is
*named* matters as much as what it does: `--shell` would read, a year from now,
like a flag that does nothing, and it is still accepted (and ignored) so that a
shortcut written before the switch keeps opening what it opened.

- [x] G82: a plain run is the new shell, and the shell this rewrite replaces runs
      only when it is asked for by name
  CHECK: cargo test -p palantir-desktop --locked --bin PalantirMC
  EXPECT: test result: ok. 593 passed
  EVIDENCE: one new test, over the function that makes the decision rather than
      over `main`, which cannot be called from a test at all: no arguments is the
      new shell, `--shell` alone changes nothing (it is the default now), a
      `--page` path does not select the old shell, and only `--classic` -- alone or
      beside another flag -- runs the old one. The reason this is worth a gate is
      not the line: it is that the line is the plan's own condition, and a
      condition nobody asserts is a condition that gets forgotten. What is
      deliberately *not* done here is the other half -- deleting `app.rs` and the
      Prism-shaped `palantir-core` modules only it calls. That is a slice of its
      own, and doing it in the same commit as the switch would make a regression
      in either one impossible to tell from the other.

### The panel's first section

The right panel was the last piece of stage 2's own work still missing, and G83
is its first section. `App.vue`'s `app-sidebar` is what is drawn: a
`--right-bar-width` column under the reference's own two-stop wash, a hairline
down its page edge (`border-l border-[--brand-gradient-border]`), and one scroll
region inside it (`app-sidebar-scrollable`) that the sections stack in. The
reference's sections are the onboarding checklist, *Playing as*, the friends
list, the fundraiser banner and the news feed; the second is the only one this
launcher can draw anything in yet, so it is the one that is there and the others
are absent rather than drawn empty -- which is the same rule the pages follow.

*Playing as* is `app.sidebar.playing-as` with `AccountsCard.vue` under it, in
the reference's own two branches. With no account: the sentence and the sign-in
button in a `rounded-xl` card at `p-3`. With accounts: an accordion -- closed by
default, which is the reference's own `open-by-default: false` -- whose header
names the account a launch would sign in as, or the reference's own *Select
account* when nothing is chosen or the chosen account is gone, and whose body is
one row per account, each with the radio mark that says which account is in
force and its own quiet red remove control (`!bg-button-bg !text-primary`,
filling red under the pointer), then the add button at the foot. The card's controls write the same `accounts.json` the other launcher
reads, which is why the file -- not the window -- is the record: an account
chosen in the panel is the account that launcher signs in as too.

Three things the reference draws that this does not, each for a reason:

1. **The player heads.** A 36px head in the header and a 24px one on every row,
   from the skin service or from the reference's own Steve asset for an offline
   account. There is no head renderer in this launcher yet -- the Skins page is
   a placeholder for the same reason -- so a row is its radio mark and its name.
2. **The sign-in flow.** The card's two Microsoft controls open a flow that is a
   later stage's; they say so in the panel instead of doing nothing, which is
   the rule the rest of this shell already holds itself to.
3. **The section's own condition.** The reference draws the section only when
   `hasLoggedIntoMinecraft` is set, and that flag belongs to the onboarding
   checklist, which is not built. The section is drawn always here, one step
   early; and since the reference's empty card *is* its picture of a launcher
   with no account, what is drawn early is the reference's own shape either way.

- [x] G83: the right panel draws the reference's first section, the accounts
      card, in both of its branches, with its own controls and notes
  CHECK: cargo test -p palantir-desktop --locked --bin PalantirMC
  EXPECT: test result: ok. 599 passed
  EVIDENCE: six new tests, five about the panel and one about the clock the
      tests themselves read. The card's rules are asserted where they are made:
      the header names the chosen uuid, the reference's *Select account*
      sentence when nothing is chosen or the choice is gone, and the offline
      account's uuid is the name's derivation (which is what makes the card name
      the same player the other launcher does). The panel test draws the
      section, both branches of the card and the note in all four themes --
      the tokens are generated per theme, and one of the four missing a value is
      exactly the failure a single-theme test cannot see -- that the accordion
      starts closed and that its header opens and closes it. A shell with no
      accounts store draws the same card and its controls are harmless, which is
      the shape the tests build. The write side is asserted on the file rather
      than on the struct: choosing an account and removing one are both re-read
      through `AccountsStore::load_with_report`, which is the other launcher's
      own view, and a uuid that is not in the file is a sentence in the panel
      rather than a silent nothing. The sign-in control's whole behaviour is
      that it says the flow is not built and the note's dismiss clears it.
      Two kit additions came with the card and are covered by the kit's own
      gate: `button_with_icon` (the reference's `Button` slot, a 20px icon at
      `gap-1.5` painted the label's ink) and `icon_button_kind`'s `Danger`
      preset, whose hover is `ButtonFrame.vue`'s `filled` interaction -- two
      colours mixed over the 150ms clock, not one colour at two brightnesses --
      and `every_control_a_page_draws_carries_its_own_key` now covers both call
      names. The sixth test is the one thing here that is not about the card:
      drawing it made an old race between tests reproduce, because the
      interaction clock was the process's and a test that navigated could
      forget a crossing another test was in the middle of asserting (the
      navigation test failed every run of the shell-and-ui suite that made it
      reproducible, and passed when run alone). The clock a test reads is now
      the test thread's own
      (`cfg(test)`), which is the honest model -- a test is its own window --
      and `a_clock_belongs_to_the_test_that_reads_it` asserts the isolation
      rather than the lock the suite used to have to remember to take.

### The four loaders, and the dialog that chooses one

Prism's repository was where this launcher read its loader builds, and Prism is
not the reference for anything here: it is a *mirror* of the four loaders' own
publications, rewritten into another launcher's shape. G84 reads each loader from
the loader. The four are four shapes rather than one with a different host, which
is why the reader is four parsers and not one: Fabric and Quilt publish a list of
builds for one game version (Quilt without a `stable` flag at all, so a build's
version string has to say it), NeoForge publishes *every* version of itself on its
maven and the game's own line has to be picked out of it, and Forge promotes two
builds per game by name -- `recommended` and `latest` -- which is the pair its
users mean by stable and newest.

G85 is the choice reaching the disk. The reference's custom-setup step is the
name, the modloader chips, the game-version combobox, then which build of that
loader, and all four are drawn; what a create writes is a component with the
build's version in the instance's pack profile, which is what a launch resolves.
Two decisions are worth reading rather than inferring. **The build request
carries the question it answers**: a Fabric list arriving after the Quilt chip was
pressed would otherwise be drawn under the Quilt chip, and one line of comparison
in `Store::loader_builds`'s own future is what prevents it. **The chips are rules,
not versions**: *Stable* and *Latest* name a rule, so the build the rule comes to
is drawn beside the label -- otherwise a create's own consequence is invisible
until the instance exists.

- [x] G84: the four mod loaders' build lists are read from the loaders'
      themselves -- Fabric, Quilt, NeoForge and Forge -- newest first, each
      carrying whether its own source calls it stable
  CHECK: cargo test -p palantir-net --locked
  EXPECT: test result: ok. 213 passed
  EVIDENCE: seven unit tests over the four shapes, the ordering rule and the
      cache, plus a thirteenth `#[ignore]`d live test that was run against all
      four services for `1.21.1`: Fabric 60 builds (newest `0.19.5`), Quilt 60
      (newest `0.31.0-beta.4`, whose default is the newest *release*, `0.30.1`),
      NeoForge 60 (newest `21.1.251`), Forge two (`52.1.16`, default
      `recommended` `52.1.0`). The live run corrected an expectation of mine on
      the way in: `21.4.100-beta` *is* newer than `21.4.5`, so the ordering rule
      is numbers first and a suffix below its own release, not the reverse.
- [x] G85: the creation dialog draws the reference's own custom-setup step -- the
      modloader chips, the game version, then the loader-version chips over the
      builds *Other* offers -- and what it chooses is what the instance is
      created with
  CHECK: cargo test -p palantir-desktop --locked --bin PalantirMC
  EXPECT: test result: ok. 604 passed
  EVIDENCE: two store tests (the request reads the loader's own service and
      vanilla asks nothing; a create writes `net.fabricmc.fabric-loader` at the
      chosen build into `mmc-pack.json`) and three shell tests (the dialog asks
the loader it opens on, a list about a choice the dialog has left is dropped,
and the button waits for the loader's own answer while vanilla does not).
      Vanilla is not a request at all, and the button stays usable when a list
      came back *empty* or *failed*: those are sentences on the row, and a dialog
      a reader cannot leave is worse than one that said what happened.

### The launch surface

A run used to be visible in exactly one place: the header of the instance it
belonged to. Navigate away and the launcher was running something invisible, and
the reference does not work that way -- its `AppActionBar` sits in the status bar,
in the right-hand cluster before the sidebar toggle, so a run is watchable from
whatever page the reader is on and its stop control is always where they left it.
G86 is that surface, in that place, with the reference's own order: the download
manager's chip first, then the chip that says what is running, then a dot, the
instance's name, its stop control and the way to its logs.

The chip's level is why `LaunchEvent::Progress` is kept as *numbers* rather than
only as the sentence the instance's header shows: a bar needs a fraction and the
same fact cannot be both if only the formatted line survives. The panel that chip
opens is drawn in the layout rather than floating -- iced 0.12 has no z-order,
which is the same wall the version picker hit -- and it shows the three things the
reference shows: what is being fetched, how many of them are done, and the bytes
that has cost. The rate the reference puts beside them is deliberately absent: a
rate needs a clock and a window, and a number computed from one frame's difference
is a number that jumps.

Two things the reference draws are absent and are absent features rather than
missing pixels: the offline banner needs an online/offline source this launcher
has no opinion about yet, and the update button belongs to a self-updater it does
not have. The third is a limit of this launcher rather than of this gate: the
reference's popover lists *every* running process, and this one runs one instance
at a time -- `Shell::play` refuses while a run is in flight -- so there is one job
to list and a chip rather than a menu. What *is* asserted is the half that was
missing: the run is the launcher's, not the page's.

- [x] G86: the action bar in the head follows the run from any page, with the
      run's own level as a chip and a panel, and a stop control that exists when
      there is something to stop
  CHECK: cargo test -p palantir-desktop --locked --bin PalantirMC
  EXPECT: test result: ok. 606 passed
  EVIDENCE: two new shell tests. One starts a run, navigates away and asserts the
      run is still the store's, that the four state words are the reference's own
      (and that *Running* draws none, because a process that is up needs no
      word), and that a stop pressed while the launcher is still *preparing* is
      refused -- the child slot is empty then, and the kill has nothing to send
      it to. The other asserts the level: a quarter of three-of-twelve reads
      25%, the panel's line carries the count and the bytes, an indeterminate
      level says so in words instead of inventing a fraction, and both the level
      and the panel go when the fetch does. That refusal is a behaviour change
      with a test to match: `stopping_is_a_state_the_header_can_draw_and_a_kill_
      the_shell_can_send` now puts the game up before it presses Stop, which is
      what makes the state it asserts reachable at all.

### The view-model crate, retired

The delete's first slice, and it is the one with a floor under it. `palantir-gui`
was a crate of its own whose only dependant was the desktop and whose reason to
exist was a CLI that no longer does; its doc comment still claimed the shell and
the CLI both went through it. Three readers actually do -- `instances.rs` loads
the list to build the library's cards, `launch.rs` resolves a run through the
override gates, and `app.rs` builds one row -- so the crate could not simply go,
and the move is what it got: `crates/palantir-desktop/src/model.rs`, whole, with a
hand-written `Error` rather than a `thiserror` derive, because a dependency added
for one `Display` arm is one the crate carries from then on.

What the move exposed is the part worth reading. As a library, every one of this
model's methods was "used" and the dead-code pass could say nothing. Inside the
binary, the compiler answered the question the port has been asking all along --
*what does the launcher actually call* -- and the answer was: four list methods
the library's own page supersedes (its search and sort are `pages/home.rs`'s, per
`Library.vue`), and the whole write side of the settings model
(`new`, `global`, `global_mut`, `instance`, `is_overridden`, `set_override`, three
`set_global_*`, three `set_instance_*`, `save`). A hundred and twenty-odd lines of
API kept for a page nobody has written. They went, with the one test that only
proved them, and the two that remain assert the behaviour the readers depend on:
the gate deciding instance-from-global, and an ungated key always reading the
launcher's file.

- [x] G87: `palantir-gui` is retired -- the model moved into the desktop crate,
      its unused half deleted, and the crate dropped from the workspace
  CHECK: cargo test --workspace --all-targets --locked
  EXPECT: test result: ok. 1034 passed; 0 failed; 13 ignored
  EVIDENCE: the two readers are covered by their own suites (`instances.rs`
      builds its cards from the list, `launch.rs` resolves memory through
      `effective_memory`), and the model's own five tests came with it: a library
      target that no longer exists is a test binary that no longer runs, which is
      why the workspace's total moves rather than stays. The moved file is the
      only source in the change; `Cargo.lock` loses the package, the workspace
      manifest loses the member, and the desktop manifest loses the dependency.
      The deleted methods are named above so that a later reader who wants one
      back knows it was never wired, rather than lost.

### The shell this rewrite replaces, deleted

The switch was made at G82; this is the part that has to follow it, or what is on
`rewrite-modrinth-native` is a fork with two shells rather than a rewrite. Gone:
`app.rs` (10,483 lines -- the state, the update and the whole view tree),
`glyphs.rs` (964, the hand-drawn chrome `icons_gen.rs` replaced), `icons.rs`
(292) and `settings.rs` (2,016 -- the old settings and About pages), with the
carved Prism instance art their only reader kept (`assets/icons/*.png`, ten
files), the `--classic` entry point, and the `--page`/`--modal`/`--shot`
machinery that lived in that shell's `Start`. `main.rs` is 604 lines of window
and font setup now instead of two shells and a `Sandbox`; `assets/ATTRIBUTION`
records what happened to the art rather than describing files that are gone.

Two things the product needed from that shell could not go with it, and both are
now the new shell's:

* **The window's frame.** `native::install_hit_test` takes over
  `WM_NCHITTEST`, and that is what gives the window Windows' own resize loop
  (with its cursors, including the diagonals iced has no way to ask for) and
  Windows 11's Snap Layouts on the maximize control. The old shell was its only
  caller, and this shell has no drawn resize bands, so without it nothing would
  have resized the window at all. It is called from `Shell::handle` -- the first
  message is the earliest moment there is a window, because iced builds the
  window after `Application::new` -- the maximize rectangle is published from
  `Shell::render` and derived from the shell's own `CONTROLS_*` constants (so
  there is no measurement to keep in step), and the window's own state arrives
  through a subscription the window procedure reports into: maximizing by Snap,
  the taskbar or `Win`+`Up` is something iced is never told about, and a
  maximize glyph that goes stale is the bug that subscription exists to prevent.
  The control's click belongs to Windows once that rectangle is named, which is
  why `ToggleMaximize` now asks the window instead of inverting its own flag.
* **A capture.** `--shot PATH` is what `tools/appshot.py` and
  `tools/page_gate.py` are run through, and it is a launcher flag for the reason
  it always was: iced draws the frame, so the process that drew it is the only
  one that can hand back exactly that frame -- no occlusion, no screen capture,
  no compositor to be running. It is a settle timer, `window::screenshot` and
  `write_shot`, which writes the PNG and closes the window; `--size` still states
  the client pixels and a capture is still born off the desktop, both asserted in
  `main.rs`'s tests.

What the delete cost is visible in the warnings rather than in the tests. With
the last caller gone, 199 dead items showed themselves across the crate: 67 in
`theme.rs` (the hand-written palette, whose own deletion is stage 2's last piece),
38 in `browse.rs`, 17 in `catalog.rs`, and smaller counts in twelve more files.
They are not part of this slice -- the shell's removal does not depend on them,
and one of them cannot be pruned by hand at all: `text_gen.rs` is generated,
byte-checked output, so the allowance for the keys no page or shell paints yet
has to be emitted by `tools/gen_text.py` rather than deleted row by row. That is
the plan's next item, and it is named as such.

- [x] G88: the shell this rewrite replaces is deleted, and the frame and the
      capture it owned are this shell's own
  CHECK: cargo test --workspace --all-targets --locked
  EXPECT: test result: ok. 903 passed; 0 failed; 13 ignored
  EVIDENCE: the transcript of that command on the pushed tree:

```
$ cargo test --workspace --all-targets --locked
    168 passed; 0 failed  (palantir-core, lib)
      8 passed; 0 failed  (palantir-core, tests/compat.rs)
    479 passed; 0 failed  (palantir-desktop, bin)
      4 passed; 0 failed  (palantir-desktop, tests/native.rs)
     31 passed; 0 failed  (palantir-loader, lib)
    213 passed; 0 failed  (palantir-net, lib)
      0 passed; 0 failed; 13 ignored  (palantir-net, tests/live.rs)

$ cargo clippy --workspace --all-targets --locked -- -D clippy::correctness
the workspace compiles, nothing in the correctness-deny set fails, and 288
warnings remain -- of which 199 are the dead items above, 56 the unused keys of
the generated `text_gen.rs`, and none of them a correctness lint
```

  903 tests pass where 1034 did, and the difference is the point rather than a
  loss: the deleted modules carried 131 tests (the old shell's, and the old
  settings page's), and a test for a page that no longer exists is not coverage.
  The four files that remain in the desktop crate's `src/` that this slice did
  not touch -- the pages, the store, the generated tables -- are unaffected, and
  the two behaviours it added are asserted: the window opens undecorated,
  branded and screen-fitted (`main.rs`), and a capture states its own size and
  hides off the desktop (`main.rs`). The frame itself needs a window to be seen:
  `native.rs`'s hit-test tests are unaffected and still compare `hit_code`
  against the fixture, and the live check of the whole path is the capture below.

  A capture taken through the ported path, on this machine:

```
$ cargo build -p palantir-desktop --locked
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.04s
$ python tools/appshot.py --page home --size 1280x720 --out .scratch/shell-after-delete.png
C:\PalantirMC\.scratch\shell-after-delete.png: 1280x720, written in 16.1s (launcher exit 0)
```

  That capture holds 1,706 distinct colours, and its five commonest are
  `(22,24,28)`, `(39,41,46)`, `(52,54,60)`, `(26,35,33)` and `(22,29,28)` -- the
  reference's dark base, its raised surface and its hairlines. A blank window is
  one colour; a shell that painted is this.

### The engine's transcript, and the pushes that could not start

Every push of stages 2, 3 and 4 went to a runner that could not schedule a job.
At the time of writing, one per commit from `82232f9` onwards —
`36033443993`, `36038333030`, `36143868395`, `36152873678`, `36156018946`,
`36158909795`, `36159099493`, `36162312983`, `36162426595`, `36165962973`,
`36241262447`, `36242305444`, `36254170068`, and every push adds one — until
G112's push, which added nothing at all: the API reports `total_count 0` for its
SHA, a second shape of the same block. Each died
in three to six seconds with zero steps and
the same annotation: `recent account payments have failed or your spending limit
needs to be increased`. So no job ran, in either workflow, and there is no
`test result` line from a runner to quote for any of them. The ids are written
out because a blocked run is a fact about the account and not a verdict on the
tree, and the only way a later reader can tell the two apart is if both are named
the same way. **The block ended on 2026-09-29**: the tree gained a public mirror,
`36574682807` is the first run since G101 with `test result` lines in it, and
G113 is what the first run that could execute a job found. The commands `ci.yml` runs were run here
instead, with the same flags, on the tree that was pushed:

```
$ cargo test --workspace --all-targets --locked
    168 passed; 0 failed  (palantir-core, lib)
      8 passed; 0 failed  (palantir-core, tests/compat.rs)
    610 passed; 0 failed  (palantir-desktop, bin)
      4 passed; 0 failed  (palantir-desktop, tests/native.rs)
     31 passed; 0 failed  (palantir-loader, lib)
    213 passed; 0 failed  (palantir-net, lib)
      0 passed; 0 failed; 13 ignored  (palantir-net, tests/live.rs)

$ cargo clippy --workspace --all-targets --locked -- -D clippy::correctness
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 11.75s
    (incremental, on a tree this machine had already built; the same command
     cold takes minutes, which is what the runner pays)
    (84 warnings, every one warn-by-default; none of them in the files stages 2,
     3 or 4 added, and most of them in the shell this rewrite deletes)

$ python tools/gen_theme.py --check && python tools/gen_icons.py --check && python tools/gen_text.py --check && python tools/gen_tokens.py --check
theme generation is byte-identical
icon generation is byte-identical
text generation is byte-identical
token generation is byte-identical
```

1034 tests pass, nothing in the correctness-deny set is failing, and all four
generated files match their sources. The desktop's own line was 554 when the
engine's slices landed and is 610 now: the seam (G75), the interaction (G76), the
settings pane (G77), stage 5's three flows (G78-G80), the launch (G81), the
switch (G82), stage 3's panel (G83), the loader read and the dialog that chooses
one (G84-G85), the action bar (G86) and the view-model crate's retirement (G87)
added fifty-six between them, and the run above was taken after all of them. The caveat in the transcript below -- that a
local run is not a clean checkout -- applies here too, with one thing added:
**four live tests have run against the real world and passed** (G68, G70, G72
and G74),
which is a stronger kind of receipt than a local unit run. The services answering
is the only part of this document that comes from outside this machine, and both
of those tests were run here rather than by the runner, which cannot start.
Re-running the push when it can is still the first thing to do with this tree.

### The transcript of the earlier pushes, for comparison

That is what the run looks like now; this is what it looked like before the
engine's slices, quoted so the two can be read against each other. CI could not
schedule stages 2 and 3 either: runs `36038333030` and `36033443993`
both died in five seconds with zero steps — `recent account payments have
failed` — so no job ran and there is no `test result` line from a runner to quote.
The three commands `ci.yml` runs were run here instead, with the same flags, on
the tree that was pushed:

```
$ cargo test --workspace --all-targets --locked
    168 passed; 0 failed  (palantir-core)
      8 passed; 0 failed  (palantir-gui)
    554 passed; 0 failed  (palantir-desktop, bin)
      4 passed; 0 failed  (palantir-desktop, tests/native.rs)
      6 passed; 0 failed  (palantir-loader)
     31 passed; 0 failed  (palantir-net)
    104 passed; 0 failed  (palantir-net)
      0 passed; 0 failed; 7 ignored  (palantir-net, tests/live.rs)

$ cargo clippy --workspace --all-targets --locked -- -D clippy::correctness
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 29.14s
    (84 warnings, every one warn-by-default; none of them in the files stages 2
     and 3 added, and most of them in the shell this rewrite deletes)

$ python tools/gen_theme.py --check && python tools/gen_icons.py --check && python tools/gen_text.py --check
theme generation is byte-identical
icon generation is byte-identical
text generation is byte-identical
```

A local run is not a clean checkout and this document is not going to pretend
otherwise: the same compiler and the same flags on a machine that has built the
tree before is a *weaker* claim than the runner's, which is why `AGENTS.md`
makes the runner the authority. What it does say is that all 876 tests pass (the
run under G90 below), that nothing in the correctness-deny set is failing, and
that all three generated files match their sources; the run under "The engine's transcript" above, at 1015,
is the same set after the seam, the interaction, the settings pane and stage 5's
flows so far landed.
Re-running the push when the account can schedule jobs is the first thing to do
with this tree.

- [x] G89: what the deleted shell was the last caller of is gone, and the
      generated table's allowance is its generator's to emit
  CHECK: cargo test --workspace --all-targets --locked
  EXPECT: test result: ok. 871 passed; 0 failed; 13 ignored
  EVIDENCE: the transcript of these commands on the pushed tree:

```
$ cargo test --workspace --all-targets --locked
    168 passed; 0 failed  (palantir-core, lib)
      8 passed; 0 failed  (palantir-core, tests/compat.rs)
    447 passed; 0 failed  (palantir-desktop, bin)
      4 passed; 0 failed  (palantir-desktop, tests/native.rs)
     31 passed; 0 failed  (palantir-loader, lib)
    213 passed; 0 failed  (palantir-net, lib)
      0 passed; 0 failed; 13 ignored  (palantir-net, tests/live.rs)

$ cargo clippy --workspace --all-targets --locked -- -D clippy::correctness
warning: `palantir-core` (lib test) generated 10 warnings
warning: `palantir-core` (lib) generated 9 warnings (9 duplicates)
warning: `palantir-core` (test "compat") generated 1 warning
warning: `palantir-net` (lib test) generated 1 warning
warning: `palantir-net` (lib) generated 1 warning (1 duplicate)
warning: `palantir-desktop` (bin "PalantirMC") generated 3 warnings
warning: `palantir-desktop` (bin "PalantirMC" test) generated 21 warnings (2 duplicates)
46 warnings where G88's run had 288, and not one of them a dead item:
cargo clippy ... 2>&1 | grep -cE "never (used|read|constructed)" is 0

$ python tools/gen_theme.py --check && python tools/gen_icons.py --check \
    && python tools/gen_text.py --check
CONFIRMED: theme generation is byte-identical
CONFIRMED: icon generation is byte-identical
CONFIRMED: text generation is byte-identical

$ cargo build -p palantir-desktop --locked && target/debug/PalantirMC.exe --shot .scratch/after-prune.png
exit 0, 1257x707, 1,903 distinct colours, and pixel-identical to the capture
taken before this slice (`ImageChops.difference(...).getbbox()` is None)
```

  199 dead items were the delete's receipt -- G88 measured them, and this slice
  is that list worked to zero. The rule it ran on is the one the lint states
  rather than the one it looks like it states: an item whose only callers are
  themselves dead is dead, so the list was taken twice -- once for the shipped
  binary and once for the test build -- and every item was sorted by which of
  the two still had a caller for it. Dead in both: deleted. Live only in the
  tests: kept, with `#[cfg(test)]` on the item, because a test is a reader and
  deleting what it reads would delete the record of what was measured. Dead
  only in the tests: deleted, since nothing ships it and nothing reads it.
  `never constructed` was the one report not acted on blind -- a type can be
  reported that way while a live `impl` on it still compiles, and it is the
  `impl` the build would miss.

  The clearest shape of that decision is `theme.rs`, where 67 of the items
  lived: the palette fields the old shell painted with -- the chrome, the
  sidebar's wash, the modal's own surface -- and the painters that read them.
  This shell's styles read the fields it kept, so those are dead in the binary;
  the gate test that checks every field, dead or not, against the tokens it was
  transcribed from is not, so the fields and their three constructors keep them
  under `#[cfg(test)]`. 215 lines of marker went in for those and their like;
  the slice is +378/-1,842 across 21 files.

  One item could not be pruned by hand at all. `text_gen.rs` is generated and
  `ci.yml` checks it byte for byte, so the allowance for its warnings has to be
  the generator's: `tools/gen_text.py` now writes the `enum_variant_names`
  allowance with the reason beside it (the reference's own keys really do end in
  `key`), and the 53 single-character literals it emitted as `push_str` are
  `push`. Re-running the generator leaves the file identical, which is what the
  byte check asks.

  Deliberately not done: the Prism-shaped `palantir-core` modules stay, because
  every one of them still has a reader in this crate -- the flattening importer
  that replaces them is the launch surface's work, not this slice's. And the 32
  tests that left with their subjects (the modal's arrival, the version
  catalog's list builders, the old shell's GPU report) are a loss of coverage in
  one narrow sense and none at all in another: 903 tests to 871 is exactly those
  32, and a test for code that is no longer there is not coverage.

  The runner could not be the receipt again: the run for this slice's parent,
  `36292014607`, died in three seconds with zero steps -- `recent account
  payments have failed` -- so the transcript above is this machine's, run with
  the flags `ci.yml` uses. Re-running the push when the account can schedule
  jobs is still the first thing to do with this tree.

- [x] G90: the bar watches several runs at once, and the download manager's job
      list is more than the run's own job
  CHECK: cargo test --workspace --all-targets --locked
  EXPECT: test result: ok. 876 passed; 0 failed; 13 ignored
  EVIDENCE: the transcript of these commands on this tree:

```
$ cargo test --workspace --all-targets --locked
    168 passed; 0 failed  (palantir-core, lib)
      8 passed; 0 failed  (palantir-core, tests/compat.rs)
    452 passed; 0 failed  (palantir-desktop, bin)
      4 passed; 0 failed  (palantir-desktop, tests/native.rs)
     31 passed; 0 failed  (palantir-loader, lib)
    213 passed; 0 failed  (palantir-net, lib)
      0 passed; 0 failed; 13 ignored  (palantir-net, tests/live.rs)

$ cargo clippy --workspace --all-targets --locked -- -D clippy::correctness
warning: `palantir-core` (lib) generated 9 warnings
warning: `palantir-core` (test "compat") generated 1 warning
warning: `palantir-net` (lib) generated 1 warning
warning: `palantir-core` (lib test) generated 10 warnings (9 duplicates)
warning: `palantir-net` (lib test) generated 1 warning (1 duplicate)
warning: `palantir-desktop` (bin "PalantirMC") generated 3 warnings
warning: `palantir-desktop` (bin "PalantirMC" test) generated 21 warnings (2 duplicates)
46 warnings, the same profile G89's run had, none of them in the files this
slice touched, and not one of them a dead item:
cargo clippy ... 2>&1 | grep -cE "never (used|read|constructed)" is 0

$ cargo build -p palantir-desktop --locked && target/debug/PalantirMC.exe --shot .scratch/after-multirun.png
exit 0, 1257x707, 1,903 distinct colours, and pixel-identical to the capture
G89 took before this slice (`ImageChops.difference(...).getbbox()` is None)
```

  One run at a time was the whole of what the bar could be. The reference's action
  bar is built for several -- `currentProcesses`, a floating menu over them, and a
  download manager whose list is every job the app carries -- and this slice is
  the two models that were missing before either could be drawn, one of them in
  the shell and one in the store.

  The shell's: `run: Option<ActiveRunData>` and one shared `ChildSlot` became
  `runs: Vec<Run>`, each `Run` with a slot of its own. That is not tidiness, it is
  what a stop means -- a stop is aimed at an *instance*, and the kill has to reach
  the child that instance's launch put in *its* slot, which one shared slot cannot
  promise. Every `LaunchEvent` already carried a `run_id` (G81's seam, which is
  also why there was anything to route by), so `instance_of` turns an id back into
  an instance for as long as that run is the shell's, and a stale id changes
  nothing at all: the single run's rule, kept now that there is a second run to
  get it wrong for. `play` refuses the same instance twice and allows a different
  one, and the launch subscription is a batch of one channel per run.

  The store's: `launch: Launch` became `launches: BTreeMap<String, Launch>` plus a
  selected id the *reader* derives -- the stored one while it is still running,
  and otherwise the first run there is. What that buys is the chip's behaviour,
  which the reference has too: a new entry becomes the selected one, so the bar
  follows the press that started a run, and later words about that same run move
  nothing. The order is the map's, so "first" is the same run on every frame.

  The popover is the reference's own condition -- `currentProcesses.length > 1`
  draws the chevron, a list of one would be a control that does nothing -- with a
  row per running process under it: the indicator dot, the instance's name, *Star*
  on the one the bar is about, its state, a red `StopCircle` while it is running,
  and a `TerminalSquare` to its logs. The whole row is the button: pressing one is
  `SelectRun`, which is what makes the popover a way to *choose* a process rather
  than only to read them. The chevron rotates rather than swapping glyph, and the
  panel goes with the last run that ends, because a list of one is not a list.

  The download manager is the same shape one level down. `jobs` is a
  `BTreeMap<String, install::Progress>` keyed by instance, so an instance that is
  fetching is a row, two instances installing side by side are two rows, and the
  head counts two. The chip carries that count as well -- the reference's own
  `activeCount` pill, brand ink on `--color-green-highlight` -- because the chip is
  about one job and the number is what keeps the others from being invisible while
  it is the only thing on screen. The chip's *label* still falls back the way
  `shown_job` says: the selected run's phase when it has one, and otherwise the
  first run that is fetching, because a chip that went blank while another run was
  still downloading would hide work that is happening.

  Three things were deliberately not drawn. The reference's per-job controls --
  pause, resume, retry, cancel, dismiss, copy details -- have nothing behind them:
  a job here is a running launch's own phase, and the words a stopped phase would
  answer are the run's, on the chip and on the popover row, so a pause for a fetch
  that cannot pause would be a control that lies. The popover and the panel both
  float in the reference and are rows under the head here, because iced 0.12 has
  no z-order -- the wall the version picker and the create dialog already hit,
  taken the same way and written down where the code is. And **no gate has
  photographed the popover**: it draws only when a second run exists, and a
  `--shot` run cannot start two real launches. What it has instead is a test that
  renders the whole surface in that state -- two runs, the panel down,
  `drop(shell.render())` -- beside the capture above, which is the other half of
  the same claim: a plain run's surface is unchanged by any of it.

  The runner could not be the receipt again. The push before this slice,
  `1a3b367`, is run `36292534532` -- three seconds, zero steps -- and this
  slice's own push, `d166006`, is run `36295084058`, four seconds with zero steps
  and the same billing annotation. Re-running those pushes when the account can
  schedule jobs is still the first thing to do with this tree.

- [x] G91: the launch's own downloads are the engine's queue, and the digest the
      transfer checks is whatever kind the publisher stated
  CHECK: cargo test --workspace --all-targets --locked
  EXPECT: test result: ok. 879 passed; 0 failed; 13 ignored
  EVIDENCE: the transcript of these commands on this tree:

```
$ cargo test --workspace --all-targets --locked
    168 passed; 0 failed  (palantir-core, lib)
      8 passed; 0 failed  (palantir-core, tests/compat.rs)
    455 passed; 0 failed  (palantir-desktop, bin)
      4 passed; 0 failed  (palantir-desktop, tests/native.rs)
     31 passed; 0 failed  (palantir-loader, lib)
    213 passed; 0 failed  (palantir-net, lib)
      0 passed; 0 failed; 13 ignored  (palantir-net, tests/live.rs)

$ cargo clippy --workspace --all-targets --locked -- -D clippy::correctness
exit 0, and the two crates this slice touched are where they were before it:
warning: `palantir-desktop` (bin "PalantirMC") generated 3 warnings
warning: `palantir-desktop` (bin "PalantirMC" test) generated 21 warnings
and not one of them a dead item:
cargo clippy ... 2>&1 | grep -cE "never (used|read|constructed)" is 0

$ python tools/gen_theme.py --check && python tools/gen_icons.py --check \
    && python tools/gen_text.py --check
CONFIRMED: theme generation is byte-identical
CONFIRMED: icon generation is byte-identical
CONFIRMED: text generation is byte-identical
```

  The launch path had a second way out of its own. `online_backend` handed the
  worker an `OnlineMetaStore` and a `BlockingHttpFetcher`, the install phases
  fetched through `download_many_with_progress`, and the Java runtime's list,
  version file and manifest were read through the same fetcher -- so a launch's
  downloads drew a ceiling the interface's requests knew nothing about,
  restarted from zero after a dropped connection, could not be stopped from the
  window that started them, and were checked by a digest comparison written in
  the desktop after the fact. Three of those four are the engine's whole reason
  for existing, and they were not reaching the one path that moves hundreds of
  megabytes.

  `wire.rs` is the crossing: one `Wire` per launch worker, holding the engine's
  `HttpPool` under the process-wide ceiling, one `MetadataCache` in the
  launcher's own `cache/meta`, and one `Backoff`. `Wire::document` reads a
  document through the cache -- so a Java version file that has not changed costs
  a revalidation rather than a download -- and `Wire::files` puts every file on a
  `Scheduler` and drains its events, reporting each file as it lands. The install
  and the Java runtime take that instead of a fetcher; nothing in the launch path
  builds a client or a thread pool of its own any more.

  The one change this needed from the engine is the reason it is a slice rather
  than a substitution: `Download` carried `sha256: Option<String>`, and Mojang
  addresses every library, every asset object and every Java file by its `sha1`.
  A digest field that could only hold one kind meant the engine could not check
  the files a launch fetches at all -- which is why the desktop was checking them
  afterwards, reading every downloaded byte a second time. It is now
  `Digest`, the type the content store is already keyed by, so
  `Download::verified` takes whichever hex the publisher wrote and `finish`
  verifies the part file with that kind before the rename. The post-hoc checks
  went with the field: `install::verify_download` and `browse::sha1_file` are
  `#[cfg(test)]` now, kept because their tests are where the rule they state is
  written down.

  A fixture had to change shape with them, and the shape is better. The install,
  Java and launch tests used the crate's own `MapFetcher`, a hash map behind the
  old `Fetcher` trait; they now drive the engine's `MapFetch`, which speaks the
  offset contract. So the tests exercise the real queue, the real retry policy
  and the real digest check, and the two that asserted a mismatch by its old
  wording now assert the engine's -- which names the part file, the digest it
  expected and the digest it got. One new test in `wire.rs` states the claim
  itself: a 4096-byte body whose first 2048 bytes are already in the part file is
  fetched as one 2048-byte ranged request, and the file that comes out hashes to
  the whole thing.

  Deliberately not in this slice, and named where the plan is: `browse.rs`'s pack
  installer (test-only code that still holds its own client) and `resolve`'s
  metadata, which still reads Prism's mirror. The second is a change of *which
  service* is asked rather than of how the asking happens, and mixing it into a
  slice about the asking would have made both halves harder to check.

  The runner could not be the receipt again, and this is the one slice whose
  push left nothing at all to look at: `21abf30` was followed ninety seconds
  later by `57c92ba`, `ci.yml` sets `concurrency: cancel-in-progress`, and a
  superseded push leaves no run object behind -- the API answers
  `total_count: 0` for that sha where it answers `1` for every other push here.
  What the runner has of this slice is the run for that next push's tip,
  `36297604580`: four seconds, zero steps, `recent account payments have failed
  or your spending limit needs to be increased`, listed against both `Test
  workspace` and `Lint`. Re-running both pushes when the account can schedule
  jobs is still the first thing to do with this tree.

- [x] G92: the plan is a page as well as a table, and the page is checked against
      the documents it is drawn from
  CHECK: python tools/dashboard.py --check
  EXPECT: CONFIRMED: the page carries all 93 gates, 6 stage cards and every subject as written
  EVIDENCE: the transcript of these commands on this tree:

```
$ python tools/dashboard.py --check
CONFIRMED: the page carries all 93 gates, 6 stage cards and every subject as written

$ python tools/progress.py --dashboard
wrote C:\PalantirMC\.scratch\progress.html  (56,068 bytes)
```

  `progress.py` answers "how much is left" in a table that has to be re-read from
  the top; this answers it in a window. `tools/dashboard.py` draws the same two
  documents -- the stage cards with their gates and their open work, the whole
  gate ledger with every subject line, and the code the repository owns -- into
  one HTML file with its styles and its script inside it.

  Nothing is fetched. No font, no framework, no stylesheet, no favicon: a page
  that needs a network is blank exactly when the network is what is being worked
  on, which is most of this project's recent history. The four palettes are the
  launcher's own -- the `LIGHT`/`DARK`/`OLED`/`RETRO` columns of `theme_gen.rs`'s
  colour table -- so the page and the thing the page is about are painted from
  the same colours, and there is no fifth opinion about what the launcher's green
  is. The theme is remembered per browser, because this is a page left open for
  hours.

  It is interactive in the four ways a reader of this plan actually needs, and
  no others: `/` focuses the search, `Esc` clears it, `0`-`5` filters the ledger
  to a stage, and a gate chip on a stage card scrolls the ledger to that gate.
  The counts on the filter chips are live, and the ring at the top is the same
  `plan_totals` the terminal table prints -- pulled out into a function for
  exactly this reason, because a page that computed its own answer could
  disagree with the pane left open beside it.

  `--check` states what the page must carry, so that it cannot quietly stop
  carrying it: every gate in the ledger, a card for every stage, one ledger row
  per gate, and every subject present as written. It earned its place on the
  first run -- the row count came back as 91 against a page that draws 97,
  because `class="gate"` also matches `class="gate-chip"`, which is exactly the
  kind of thing a page breaks on invisibly and a reader never notices.

  The runner could not be the receipt: this slice's push, `57c92ba`, is run
  `36297604580` -- four seconds, zero steps, the same billing annotation -- and
  it is also the only run that carries G91's commit, since the two were pushed
  within ninety seconds of each other.

- [x] G93: a pack installs over the engine's queue, and the window's own crate
      names no HTTP client at all
  CHECK: cargo tree -p palantir-desktop --depth 1 -e normal --locked | grep -c reqwest
         cargo test --workspace --all-targets --locked
  EXPECT: 0
          every suite line `test result: ok`, 879 passed and 13 ignored between them
  EVIDENCE: the transcript of these commands on this tree:

```
$ cargo tree -p palantir-desktop --depth 1 -e normal --locked | grep -c reqwest
0

$ cargo tree -p palantir-desktop --depth 1 -e normal --locked
palantir-desktop v0.1.0 (C:\PalantirMC\crates\palantir-desktop)
├── futures v0.3.34
├── iced v0.12.1
├── image v0.24.9
├── md-5 v0.10.6
├── palantir-core v0.1.0 (C:\PalantirMC\crates\palantir-core)
├── palantir-loader v0.1.0 (C:\PalantirMC\crates\palantir-loader)
├── palantir-net v0.1.0 (C:\PalantirMC\crates\palantir-net)
├── serde v1.0.229
├── serde_json v1.0.151
├── sha1 v0.10.7
├── wgpu v0.19.4
└── windows-sys v0.52.0

$ cargo test --workspace --all-targets --locked
    168 passed; 0 failed; 0 ignored  (palantir-core, lib)
      8 passed; 0 failed; 0 ignored  (palantir-core, tests/compat.rs)
    455 passed; 0 failed; 0 ignored  (palantir-desktop, bin)
      4 passed; 0 failed; 0 ignored  (palantir-desktop, tests/native.rs)
     31 passed; 0 failed; 0 ignored  (palantir-loader, lib)
    213 passed; 0 failed; 0 ignored  (palantir-net, lib)
      0 passed; 0 failed; 13 ignored  (palantir-net, tests/live.rs)

$ cargo clippy --workspace --all-targets --locked -- -D clippy::correctness
exit 0; `palantir-desktop` (bin) 3 warnings, (bin test) 20, none of them in the
file this slice touched, and `grep -cE "never (used|read|constructed)"` is 0

$ cargo build -p palantir-desktop --locked && target/debug/PalantirMC.exe --shot .scratch/after-g93b.png
exit 0, 1257x707, 1,903 distinct colours, and pixel-identical to the capture
G90 took (`ImageChops.difference(...).getbbox()` is None): the window is not
what this slice moved, and the crate it is drawn from had one dependency fewer
when it was taken
```

  `browse.rs`'s pack installer was the last thing in the launcher that fetched a
  file with machinery of its own: it built a `reqwest` client, wrapped it in the
  old `BlockingHttpFetcher`, and handed the pair to `download_many_with_progress`
  -- a second connection pool, a second answer to how many requests this launcher
  makes, and a `.part`-then-rename written by hand in `fetch_one`. Installing a
  pack is now one `Wire::files` call for the files' primary URLs and, for the rare
  file whose host would not serve it, one more job per mirror on the same queue.

  Everything the old path did itself is now the engine's: the resume, the digest
  check before the rename, the deletion of a part file that fails it, the retry
  policy and the process-wide ceiling. What went with it is this module's own
  byte-checker (`sha1_hex`, `sha1_file`, `verify_download`), the client builder,
  and the `USER_AGENT` constant that existed because the metadata fetcher could
  not set one -- `engine::http` already sends the same string on the client every
  request goes through, so nothing a service sees changed.

  One check stayed here, because it is the caller's and not the transfer's: the
  *length* the API published. It is the only check a file with no digest has, and
  it is measured on the file that landed rather than counted off the wire -- a
  second install of the same file transfers nothing at all, so a byte count from
  the wire would read as an empty file. `installed.bytes` is that measurement.

  Two tests state what the single-file install does now, one request and the file
  measured where it lands, and one states what the length check is still for: a
  body of the wrong length is deleted rather than left where the game would load
  it. The pack test drives the engine's own `MapFetch` through `wire::Script` and
  asserts the engine's wording for a mismatched file -- `hash mismatch`, naming
  the part file and both digests -- which is the receipt that the check happens in
  the transfer rather than after it.

  The crate's `reqwest` dependency came out of `Cargo.toml` with the last of its
  users, which is what the `CHECK` above is: the crate that draws the window no
  longer has an HTTP client to name. `palantir-net` still depends on it, and that
  is the point -- one client, in the engine.

  Deliberately not in this slice: `palantir_net::download`, the blocking pipeline
  the engine replaced, now has no caller outside its own tests and the live
  tests' `verify_sha256`. It is named rather than deleted here because deleting it
  means re-sourcing that live check, and a slice about the pack installer is not
  where a live receipt should change shape.

  The runner could not be the receipt again: the push before this slice,
  `b46d59f`, is run `36298415367` and this slice's own push, `04a84e0`, is run
  `36299906456` -- both of them `Test workspace` and `Lint` dying in two seconds
  with zero steps and `recent account payments have failed or your spending
  limit needs to be increased`, and both of them leaving `Live services` and
  `Build exe` unscheduled. The transcript above is this machine's, run with the
  flags `ci.yml` uses.

- [x] G94: a loader's launch profile comes from the loader's own service, and
      what is left on Prism's mirror is measured rather than assumed
  CHECK: cargo test --workspace --all-targets --locked
         cargo test -p palantir-desktop --locked -- meta::tests::a_fabric_instance_resolves_its_loader_from_fabric
  EXPECT: test result: ok. 884 passed; 0 failed; 13 ignored
          1 passed, and it created a real instance for it
  EVIDENCE: the transcript of these commands on this tree:

```
$ cargo test --workspace --all-targets --locked
    168 passed; 0 failed  (palantir-core, lib)
      8 passed; 0 failed  (palantir-core, tests/compat.rs)
    458 passed; 0 failed  (palantir-desktop, bin)
      4 passed; 0 failed  (palantir-desktop, tests/native.rs)
     31 passed; 0 failed  (palantir-loader, lib)
    215 passed; 0 failed  (palantir-net, lib)
      0 passed; 0 failed; 13 ignored  (palantir-net, tests/live.rs)

$ cargo test -p palantir-desktop --locked -- meta::tests::a_fabric_instance_resolves_its_loader_from_fabric
running 1 test
test meta::tests::a_fabric_instance_resolves_its_loader_from_fabric ... ok
test result: ok. 1 passed; 0 failed; 0 ignored

$ cargo clippy --workspace --all-targets --locked -- -D clippy::correctness
exit 0; `palantir-net` (lib) 1 warning, `palantir-desktop` (bin) 3 and (bin
"PalantirMC" test) 21, and `grep -cE "never (used|read|constructed)"` is 0
```

  The last stage-4 item was named as one thing -- "the metadata source `resolve`
  reads" -- and it is three, because the mirror was measured before it was moved.
  `meta.prismlauncher.org` is not a copy of the publishers' files: Prism fetches
  each one, rewrites it, and serves the rewrite. Two of the rewrites can be
  served from the publisher directly, and one of them landed here.

  **Fabric and Quilt.** Their own services publish a launch profile per *game
  version*: `meta.fabricmc.net/v2/versions/loader/{game}/{build}/profile/json` and
  `meta.quiltmc.org/v3/versions/loader/{game}/{build}/profile/json`. That document
  is Mojang's shape -- `id`, `inheritsFrom`, `mainClass`, `libraries`, and
  `arguments` -- with no `order`, which is Prism's addition to the same file, so it
  parses the way piston's version files do. Measured against Prism's copy of the
  same build: the loader's own file has the *mappings jar among its libraries*
  (`net.fabricmc:intermediary:1.21.4`, `org.quiltmc:hashed:1.21.4`), where Prism
  splits it into a second component and states a `requires` for it. An instance
  this launcher creates lists two components, so the difference is not cosmetic:
  the mappings reach the classpath because the loader says so, not because another
  launcher's server wrote a requirement.

  `crates/palantir-desktop/src/meta.rs` is the store `resolve` is handed now: one
  instance's game version read off its pack profile, the loaders' own profiles
  over the wire's cache and client (`Wire::loaders` is the handle), and Prism's
  mirror beside them for everything else. The route is one function --
  `PublisherMeta::source` -- so "what is still on the mirror" is readable rather
  than implied, and it is tested as a table: Fabric and Quilt to the publisher,
  `net.minecraft`, `net.minecraftforge`, `net.neoforged` and
  `net.fabricmc.intermediary` to the mirror. An instance that names no game
  version is its own case and takes the mirror too, rather than failing a launch
  over a URL that cannot be built.

  **What the measurement says is left, and why it is two more slices rather than
  a URL.** `net.minecraft`'s mirror file is Mojang's rewritten: `arguments.game`
  and `.jvm` flattened into the legacy `minecraftArguments` string, `javaVersion`
  into `compatibleJavaMajors`, `downloads.client` into `mainJar`, plus `+traits`
  (`XR:Initial`, `FirstThreadOnMacOS`, the quick-play features). This launcher's
  version-file model reads the translated shape and has no reading of Mojang's
  `arguments` at all, so piston's file is a translation to write rather than a
  source to switch. Forge's and NeoForge's is a different one: their profile is
  inside an installer jar whose processors patch the client and unpack maven
  artifacts, and Prism's copy is a rewrite around ForgeWrapper, a third-party
  project that runs those processors at launch. Both are now bullets in
  `NEXT_STEPS.md` with those measurements in them.

  Two things the loader's own file carries that this launcher still does not read
  are named rather than discovered later: a per-library digest (Fabric and Quilt
  put `sha1` at the top level of a library entry; this model reads it under
  `downloads.artifact`, and Prism's copy drops it too, so nothing regressed) and
  Fabric's own `-DFabricMcEmu` JVM argument (the mirror's copy does not have it
  either). Both are in the module's documentation, because a reader who measures
  them again has spent an afternoon on a sentence.

  The runner could not be the receipt: this slice's push, `e49eb11`, is run
  `36301407378`, and the record commit before it, `6b7fc10`, is run
  `36299945287` -- both of them `Test workspace` and `Lint` dying in two seconds
  with zero steps and `recent account payments have failed or your spending
  limit needs to be increased`, and both of them leaving `Live services` and
  `Build exe` unscheduled. The record commit after it, `516b8fc`, is run
  `36301457725` -- the same block once more, three seconds and zero steps.

- [x] G95: Minecraft's own version file is read from piston, and the translation
      the mirror was doing is measured against the mirror
  CHECK: cargo test --workspace --all-targets --locked
         cargo test -p palantir-net --test live --locked -- --ignored --test-threads=1 the_translation_agrees_with_the_mirror_the_shell_read
  EXPECT: test result: ok. 895 passed; 0 failed; 14 ignored, between the seven suites
          1 passed, and it compared the two services' answers field for field
  EVIDENCE: the transcript of these commands on this tree:

```
$ cargo test --workspace --all-targets --locked
    177 passed; 0 failed  (palantir-core, lib)
      8 passed; 0 failed  (palantir-core, tests/compat.rs)
    459 passed; 0 failed  (palantir-desktop, bin)
      4 passed; 0 failed  (palantir-desktop, tests/native.rs)
     31 passed; 0 failed  (palantir-loader, lib)
    216 passed; 0 failed  (palantir-net, lib)
      0 passed; 0 failed; 14 ignored  (palantir-net, tests/live.rs)

$ cargo clippy --workspace --all-targets --locked -- -D clippy::correctness
exit 0; `palantir-core` (lib) 9 warnings, (bin test) 10, `palantir-net` 1,
`palantir-desktop` 3 and 22 -- none of them in a line this slice added, and
`grep -cE "never (used|read|constructed)"` is 0

$ cargo test -p palantir-net --test live --locked -- --ignored --test-threads=1 the_translation_agrees_with_the_mirror_the_shell_read
test the_translation_agrees_with_the_mirror_the_shell_read ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 13 filtered out
```

  The mirror was measured before it was moved, and the measurement said the
  mirror's copy of `net.minecraft` is not a copy: Prism fetches Mojang's file,
  rewrites it, and serves the rewrite. `arguments` -- the object with `game` and
  `jvm` -- becomes the legacy `minecraftArguments` string, `javaVersion` becomes
  `compatibleJavaMajors` and `compatibleJavaName`, `downloads.client` becomes a
  `mainJar` library named `com.mojang:minecraft:<id>:client`, and `+traits` is
  added. This launcher's `VersionFile` reads that translated shape and has no
  reading of Mojang's `arguments` at all, so pointing `resolve` at piston meant
  writing the translation down: `crates/palantir-core/src/version/mojang.rs`, with
  the tables in its documentation, and `PistonMeta::translated` applying it to the
  document that has already been checked against the manifest's `sha1`.

  Six versions were measured field by field rather than one -- `1.21.4`,
  `1.21.1`, `1.19.4`, `1.16.5`, `1.12.2`, `1.6.4` -- because the shapes differ and
  the two ends of that list are different documents: `1.13` and later publish
  arguments as an object, `1.6.4` and `1.12.2` publish one legacy string, and the
  Java the version needs is stated only from `1.17` on. Two of the measurements
  changed this launcher rather than the translation.

  **The arguments the launcher cannot fill.** Every version from 1.19 publishes
  `--clientId ${clientid} --xuid ${auth_xuid}`, and Prism drops that pair, for a
  reason that shows up only when the two substitution tables are read together:
  `process_minecraft_args` fills the profile's eight tokens and the session's six,
  and `replace_tokens` leaves an unknown token *as it found it*, so the pair would
  reach the game as `--clientId` followed by the literal string `${clientid}` --
  and would read to the launcher's own log redaction as two secrets. Those two
  tokens are the only ones in piston's whole list that no launch fills, measured
  across all six versions; the translation now drops an argument whose value is a
  token outside `launch::FILLED_TOKENS` together with the flag in front of it, and
  a new test in `launch.rs` states what that list is from both ends -- with a
  session nothing in it survives, and without one exactly the session's six do --
  so a token added to the list without a function that fills it fails there rather
  than at a launch.

  **Natives are their own entries now.** `1.19` introduced
  `org.lwjgl:lwjgl:3.3.3:natives-windows` -- a native with its classifier in the
  version slot and no `natives` map anywhere -- and the model read those as
  ordinary jars, which put a native library on the classpath and left the shared
  objects inside it where the JVM never looks. `Library::is_native` now answers for
  both shapes, `compatible_native` returns an entry's own classifier first, and
  `install.rs` matches a native's artifact only against the path the metadata named
  for it; `to_json` still writes the `natives` map only for the classic shape, so a
  classic file round-trips unchanged. Measured: piston's `1.21.1` lists 48 such
  entries, and the mirror's copy of the same version lists none.

  **The consequence no reading of the routing table would have found: `org.lwjgl3`.**
  Prism's `net.minecraft` carries no LWJGL at all -- 0 of its 41 libraries for
  `1.21.1`, 0 of 57 for `1.21.4` -- because it serves those from a component of
  its own and names it: `requires: [{"uid": "org.lwjgl3", "suggests": "3.3.3"}]`,
  whose file holds 72 libraries. This launcher copied that shape into every
  instance it creates (`PackProfile::vanilla` writes the `org.lwjgl3` slot with no
  version), so the slot's version came off that `requires` -- which piston's file
  does not have. Left alone, the slot would have become the hard error
  "no version is pinned and no other component requires one", which blocks a
  launch, for every instance on disk. Two changes, both about the same fact:
  `resolve` reports a versionless slot whose libraries another loaded file already
  carries as a component with nothing behind it instead of an error, keyed on the
  data (`org.lwjgl3`'s libraries are the `org.lwjgl` group) rather than on a uid
  list, so a profile still pointed at the mirror resolves the slot exactly as
  before -- `compat.rs`'s Fabric instance, whose fixture file still carries the
  `requires`, is that contrast case and stayed green -- and `vanilla` writes one
  component, because against Mojang's file there is nothing to put in the second.
  The unit test that states the new rule also states what it is worth on a
  classpath: LWJGL appears once.

  What is deliberately not translated is named in the module's documentation,
  because each one is a decision rather than an omission: `arguments.jvm` (Prism's
  own files carry `+jvmArgs: []` for all four modern versions measured -- the
  launcher builds `-Djava.library.path`, the classpath and its platform
  workarounds itself), the conditional `arguments.game` entries (the demo flag, the
  window size, the quick-play destinations, whose *features* become the traits
  `launch.rs` already keys the quick-play flags off), and `logging`,
  `complianceLevel` and `minimumLauncherVersion`, for which the model has no field.

  The live test is the receipt the fixtures cannot be: it asks piston and the
  mirror for the same version and compares the two answers, so a field the mirror
  moves and the code does not is a failure rather than a rereading. They agree on
  the main class, the argument string -- `--clientId` included, since both drop
  it -- the Java majors and name, the asset index field for field, and the client
  jar's coordinate, digest, size and URL; the traits agree once the mirror's own
  `XR:Initial` is set aside, and every library the mirror serves is asserted to be
  among the translation's, with the difference asserted to *be* LWJGL: the
  translation carries `org.lwjgl` libraries and the mirror's file carries none. A
  desktop-level test drives the same claim through a real instance and a scripted
  service: a vanilla instance resolves with no problems while the mirror's base
  URL is one the service has nothing at, which is what makes "from piston alone"
  measurable rather than a reading of the routing table.

  The runner could not be the receipt again: this slice's push, `0508b03`, is run
  `36305229876` -- `Test workspace` and `Lint` dying in four seconds with zero
  steps and `recent account payments have failed or your spending limit needs to
  be increased`, with `Live services` and `Build exe` skipped rather than
  scheduled. The transcript above is this machine's, run with the flags `ci.yml`
  uses.

- [x] G96: the project page is served by the engine, three documents behind one
      `Load`
  CHECK: cargo test --workspace --all-targets --locked
         cargo test -p palantir-net --test live --locked -- --ignored --test-threads=1 the_live_modrinth_api_answers
  EXPECT: test result: ok. 901 passed; 0 failed; 14 ignored, between the seven suites
          1 passed, and it read the project and its team from the live service
  EVIDENCE: the transcript of these commands on this tree:

```
$ cargo test --workspace --all-targets --locked
    177 passed; 0 failed  (palantir-core, lib)
      8 passed; 0 failed  (palantir-core, tests/compat.rs)
    464 passed; 0 failed  (palantir-desktop, bin)
      4 passed; 0 failed  (palantir-desktop, tests/native.rs)
     31 passed; 0 failed  (palantir-loader, lib)
    217 passed; 0 failed  (palantir-net, lib)
      0 passed; 0 failed; 14 ignored  (palantir-net, tests/live.rs)

$ cargo clippy --workspace --all-targets --locked -- -D clippy::correctness
exit 0; `palantir-core` (lib) 9 warnings, (bin test) 10, `palantir-net` 1,
`palantir-desktop` 3 and 23 -- none of them in a line this slice added, and
`grep -cE "never (used|read|constructed)"` is 0

$ cargo test -p palantir-net --test live --locked -- --ignored --test-threads=1 the_live_modrinth_api_answers
test the_live_modrinth_api_answers_a_typed_search_and_a_version_list ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 13 filtered out
```

  Discover's search was the only page that asked the engine for anything, and the
  seam it built was one function wide: a page describes what to ask for out of
  `update` or `opening`, the shell carries it to the engine off the frame thread,
  and the answer comes back as a message of that page's own. The project page is
  the second user of it, and the second user is what turned the shell's
  `Option<discover::Asked>` into an enum: a page's request is a *value* only that
  page can build, so widening the one type would have made every arm of the
  shell's match talk about Discover. `Asked { Search, Project }` is two lines, and
  each one travels back to its own page as its own message.

  Modrinth splits one project three ways, and the page is one `Load`, so the
  split is joined where the page cannot see it: `ModrinthApi::project` (the
  document the header is) and `ModrinthApi::members` (the people, which the
  document does not name) are new and cached under their own URLs at the metadata
  default TTL, the version list was already there, and `Store::project` makes the
  three calls and answers with the one thing the page draws. The translation is
  the page's (`Project::from_api`), for `Hit::from_api`'s reason: what a page *is*
  belongs to the page that draws it. A team that cannot be read is the one failure
  that does not fail the page -- the author is a caption under the title, and a
  project with no caption is still a project -- and a cached page revisited costs
  no request at all, which the store's test asserts by counting three and then
  three again.

  **The measurement that changed the code, and the fixture that would have
  agreed with the wrong rule.** The first version of the byline looked for a team
  member whose role was `Owner` and fell back to the first member. The live
  service says there is no such role: Sodium's team comes back as `Maintainer`,
  `Project Lead`, `Maintainer`, and *every* member carries `ordering: 0`, so
  neither the order nor the vocabulary spells out an owner the way the guess
  assumed -- and the guess would have credited the first maintainer. What the
  API does publish is the rank it assigns to the account that owns the project,
  so that is what is credited now: the `Project Lead` when the team has one, then
  the first member. The unit fixture was rewritten from the measured shape for
  the same reason the rule changed: a fixture written from the guess would have
  passed the whole time, which is what the live test above exists to catch.

  Two things about the page are named rather than left to be discovered. Its
  **Install** button still says what arrives later, and that is a bullet of its
  own in `NEXT_STEPS.md` now: the version list the button would choose from is on
  the page already, and what is missing is the transfer into an instance. And the
  long description is still drawn as paragraphs of its own text rather than as
  rendered markdown, which the page's module documentation has said since the
  page was drawn and which this slice did not change.

  The runner could not be the receipt: this slice's push, `15c1fb4`, is run
  `36308044463` -- `Test workspace` and `Lint` dying in three seconds with zero
  steps and `recent account payments have failed or your spending limit needs to
  be increased`, and `Live services` and `Build exe` skipped rather than
  scheduled: the same block as the seven before it. The transcript above is this
  machine's, run with the flags `ci.yml` uses.

- [x] G97: a project's own file is installed into the instance the reader picks
  CHECK: cargo test --workspace --all-targets --locked
         cargo clippy --workspace --all-targets --locked -- -D clippy::correctness
  EXPECT: test result: ok. 910 passed; 0 failed; 14 ignored, between the seven suites
          exit 0 for clippy, with no warning in a line this slice added
  EVIDENCE: the transcripts of these commands on this tree, and of the live
            measurement the rule was chosen by:

```
$ cargo test --workspace --all-targets --locked
    177 passed; 0 failed  (palantir-core, lib)
      8 passed; 0 failed  (palantir-core, tests/compat.rs)
    473 passed; 0 failed  (palantir-desktop, bin)
      4 passed; 0 failed  (palantir-desktop, tests/native.rs)
     31 passed; 0 failed  (palantir-loader, lib)
    217 passed; 0 failed  (palantir-net, lib)
      0 passed; 0 failed; 14 ignored  (palantir-net, tests/live.rs)

$ cargo clippy --workspace --all-targets --locked -- -D clippy::correctness
exit 0; `palantir-core` (lib) 9 warnings, its `compat` test 1 and its lib test
10; `palantir-net` (lib) 1 and its lib test 1 duplicate; `palantir-desktop` (bin)
3 and (bin test) 21 -- none of them in a line this slice added, and
`grep -cE "never (used|read|constructed)"` is 0. `large_enum_variant` is 0 for
the first time: the one at `pages/project.rs` was this slice's to fix, and
boxing the one `Project` a page message carries is what fixed it.

$ python - <<'PY'          # the choice, asked of the live service twice
# for each game version a project supports: the first version matching
# (game version, loader) against the first *release* matching the same, then
# the primary file of the version the first rule picks, downloaded and hashed.
PY
AANobbMI versions: 256                  (Sodium)
AANobbMI game versions: 41
AANobbMI game versions where first-match and newest-release differ: 9
AANobbMI   e.g. 1.18 -> mc1.18-0.4.0-alpha5 vs None
AANobbMI the rule picks for 1.21.4 + fabric : mc1.21.4-0.6.13-fabric release
AANobbMI   file: sodium-fabric-0.6.13+mc1.21.4.jar 1306799 bytes
AANobbMI   sha1: c881d2db971207c396b5629632437f1520c0c478
AANobbMI   downloaded: 1306799 bytes; sha1 c881d2db... matches
P7dR8mSH versions: 1203                  (fabric-api)
P7dR8mSH game versions: 389
P7dR8mSH game versions where first-match and newest-release differ: 298
P7dR8mSH   e.g. 1.14-pre1 -> 0.2.7+build.122 vs None
P7dR8mSH the rule picks for 1.21.4 + fabric : 0.119.4+1.21.4 release
P7dR8mSH   file: fabric-api-0.119.4+1.21.4.jar 2149128 bytes
P7dR8mSH   downloaded: 2149128 bytes; sha1 1c7871b6... matches

$ python - <<'PY'          # what those disagreements *are*
PY
AANobbMI differ: 9 = no-release 7 + newer-non-release 2 + other 0
P7dR8mSH differ: 298 = no-release 295 + newer-non-release 3 + other 0
```

  **The rules, and where they had to live.** Every install rule this launcher
  has ever had was in `browse.rs`, and `browse.rs` is `#[cfg(test)]` -- the old
  shell's module -- so what the launcher had was tests agreeing with code the
  binary could not call. The button did not need a rewrite; it needed a move, and
  the move is the slice. `install.rs` gained the choice (`preferred_version`) and
  the transfer (`install_file`) as live functions beside `plan`/`run`; `route.rs`
  gained `ProjectType::target_folder`, the one place a kind decides a directory
  (`mods`, `plugins`, `resourcepacks`, `datapacks`, `shaderpacks`, and nothing
  for a pack or a server); `store.rs` gained `Store::install_project`, blocking on
  one thread and a channel for `Store::search`'s reason -- no page holds a
  runtime -- and reading the project and its version list through the engine's
  own cache; the shell gained `Modal::Install` with a row per instance, and the
  page reports its press as `Ask::Install { id, title }` rather than doing
  anything itself, because *which instances exist* and *where their folders are*
  are not facts a page holds.

  **The rule is a measurement, and the reading that looks safer is wrong.** The
  version to install is the *first* one in `/v2/project/{id}/version` that matches
  the instance -- the reference's `findPreferredVersion`, and deliberately not a
  ranking by release type. The service answers publish-date descending (measured
  on Sodium's 256 versions, every `date_published` strictly ordered), so the first
  match *is* the newest, and the type is a label rather than a preference. The
  rule this replaces, `browse::pick_version`, ranked `release`, then `beta`, then
  `alpha`, which installs an older release over the newer beta the reader is
  looking at: the two rules disagree on **9 of Sodium's 41 game versions and 298
  of fabric-api's 389**, and almost every one of those is a game version with no
  release at all -- 7 of the 9 and 295 of the 298, where the release-only rule
  answers `None` and refuses to install anything, and the rest are a game version
  whose newest build is a beta, which the release-only rule would fill with an
  older release (`mc1.18-0.4.0-alpha5` and `0.2.7+build.122` are the first of
  each).
  A mod matches its instance's loader, and a mod its author also published as a
  data pack is found on the second pass (`datapack` in `loaders`, the reference's
  own fallback); anything that is not a mod is matched on the game version alone,
  because a resource pack or a shader has no loader to be wrong about. One thing
  this rule adds to the reference's: a version with no file is skipped in both
  passes, because a version that cannot be downloaded is not an answer to
  "install this", and the next one down the list usually is.

  The transfer is one file over the launcher's one queue, through the same
  `Wire` the launch uses -- `FileJob` with the published `sha1`, so the engine's
  resume, digest check before the rename and process-wide ceiling all apply, and a
  mod and the launch that will load it are one connection pool apart. The
  published *size* is checked where the file landed rather than counted off the
  wire, and a file that is already there and already right transfers nothing: the
  store's test presses install twice and counts three requests both times (the
  document, the version list, the file), and the first press is asserted against
  the file's own bytes, name, path and digest flag. A published name is a
  stranger's string joined onto a directory under the instance root, so
  `safe_file_name` refuses a separator or a `..` outright and replaces Windows'
  reserved characters, because real shader packs carry them (`Complementary:
  Reimagined` is not an attack). A pack is refused *by name* instead of by
  silence -- "Sodium is a modpack, and one of those becomes an instance of its own
  rather than a folder inside one" -- and that refusal, not the mod-shaped
  install, is what stage 3 still owes.

  **A test double that was reading another run's answers, found because one test
  failed one run in ten.** `wire.rs`'s `Script` numbers its cache directories
  from a per-process counter, and the engine's metadata cache names a directory
  after the URL's hash with hours-long TTLs -- so the nth `Script::wire()` of a
  run landed on the directory the nth call of the *previous* run had left behind.
  Measured before it was fixed: twenty directories holding `java21.json`, two of
  them with different bodies. `java_runtime::tests::a_manifest_that_does_not_match_its_published_digest_is_refused`
  failed about one run in ten and passed alone, which is the signature of a test
  that is reading somebody else's fixture: a double that outlives the test that
  wrote it agrees with whatever it finds, the same failure a stale fixture makes
  permanent. `Script::wire` removes its directory before it builds over it now.

  The runner could not be the receipt: this slice's push, `7a943aa`, is run
  `36328010219` -- `Test workspace` and `Lint` dying in two seconds with zero
  steps and `recent account payments have failed or your spending limit needs to
  be increased`, and `Live services` and `Build exe` skipped rather than
  scheduled: the same block as the eight before it. The transcripts above are this
  machine's, run with the flags `ci.yml` uses.

- [x] G98: a modpack is installed as an instance of its own
  CHECK: cargo test --workspace --all-targets --locked
         cargo clippy --workspace --all-targets --locked -- -D clippy::correctness
  EXPECT: test result: ok. 913 passed; 0 failed; 14 ignored, between the seven suites
          exit 0 for clippy, with no warning in a line this slice added
  EVIDENCE: the transcripts of these commands on this tree, and of the live pack
             the rule was checked against:
- [x] G99: the two Forge-shaped loaders' own installers are read from their own
      maven, and the launch profile inside each is translated into the shape
      `resolve` merges
  CHECK: cargo test --workspace --all-targets --locked
         cargo test -p palantir-net --test live --locked -- --ignored --test-threads=1 the_installers_profile
  EXPECT: test result: ok. 917 passed; 0 failed; 15 ignored, between the seven suites
          1 passed, and it compared each installer's file with the mirror's field for field
  EVIDENCE: the transcript of these commands on this tree:

```
$ cargo test --workspace --all-targets --locked
    177 passed; 0 failed  (palantir-core, lib)
      8 passed; 0 failed  (palantir-core, tests/compat.rs)
    476 passed; 0 failed  (palantir-desktop, bin)
      4 passed; 0 failed  (palantir-desktop, tests/native.rs)
     31 passed; 0 failed  (palantir-loader, lib)
    217 passed; 0 failed  (palantir-net, lib)
      0 passed; 0 failed; 14 ignored  (palantir-net, tests/live.rs)

$ cargo clippy --workspace --all-targets --locked -- -D clippy::correctness
exit 0; 42 warnings between the crates, the same count as the slice before --
`palantir-core` (lib) 9, its `compat` test 1 and its lib test 10; `palantir-net` 1
and 1 duplicate; `palantir-desktop` (bin) 3 and (bin test) 21 -- none of them in a
line this slice added, and `grep -cE "never (used|read|constructed)"` is 0.

$ python - <<'PY'          # the rule, against the live service
PY
Fabulously Optimized | modpack | 473 versions
  the rule picks: 14.1.0 release -> Fabulously.Optimized-v14.1.0.mrpack 167784 bytes
  sha1: a972f29de7a636e21ebbcff505ffdb335da6e3e1
  downloaded: 167784 bytes
  index: Fabulously Optimized | minecraft 26.2 | loaders {'fabric-loader': '0.19.5'} | 51 files to fetch
  overrides dir: overrides
Cobblemon Official Modpack [Fabric] | modpack | 12 versions
  the rule picks: 1.8.1 release -> Cobblemon Modpack [Fabric] 1.8.1.mrpack 100909813 bytes
  sha1: d889988c972796bc991c69ce1a7e971bf1c17559
  (the archive is 100 MB, so the rule and the published digest are the receipt)
```

  The last of the install rules, and the same move as the last slice: `browse.rs`
  held `newest_version`, `PackFetch`, `fetch_pack_files`, `install_pack_archive`
  and `import_and_fetch` in a `#[cfg(test)]` module, so none of them was reachable
  from the binary. They are live in `install.rs` now, with their tests, and the
  old module is down to the two things its own tests are about (the tab strip's
  kind table and the search parser).

  **The one install whose version is not asked of an instance.** A pack carries
  its own Minecraft version and its own loaders in its index, so there is nothing
  to match against: the rule is the newest version that *has* a file, releases
  before betas before alphas, whatever game version it names -- the reference's
  list, and the rule the old shell's `newest_version` held. Measured live rather
  than argued: Fabulously Optimized has 473 versions, the rule picks `14.1.0`, and
  the archive's own index names Minecraft **26.2** with `fabric-loader 0.19.5`, 51
  files to fetch and an `overrides/` tree. A matching rule would have refused that
  pack on any instance older than 26.2, which is the failure this rule exists to
  avoid.

  The tail is a cache and an unpack. The archive is fetched by the same rule that
  fetches any single file -- `install::fetch_pack_archive` is `install::install_file`
  pointed at `cache/meta/packs/`, so the published `sha1` is checked before the
  rename and a second install of the same version is a read of a file already
  verified -- and then the loader crate makes the instance: `modrinth.index.json`'s
  `dependencies` become `mmc-pack.json`, its `overrides/` tree is written at the
  instance root with the prefix stripped, and the 51 files go over the launcher's
  one `Wire`, mirrors tried in order, each checked against its own digest.
  `Store::install_pack` is the blocking call that drives it, `store::Outcome` is
  what tells a finished install's two shapes apart (a line for a page, or an
  instance to leave the reader in), and the shell's dialog draws one action
  instead of the instance list -- because for a pack there is no list to draw, and
  the reference's answer to the same question is to make the instance and open it.

  **Two things the tests caught, both about the shape of the tail.** The first is
  a fixture that agreed with a wrong assertion: the shell's test helper wrote zip
  entries as `overrides/<path>`, so the install test's path assertion was looking
  for `overrides/config/...` under the instance root -- where it would have been
  if the loader had *not* stripped the prefix. The loader strips it (measured in
  the store's own scratch directory: `instances/Cobblemon/config/cobblemon.json`),
  so the helper now takes the zip entry's whole name and the assertion looks where
  the file really lands. The second is the one thing a pack outcome cannot do: a
  line for the page. The reader is moved to the instance the pack made, so there
  is no page left to draw a notice on -- `store::Outcome` is what lets the shell
  hand the line back for a file install and *not* for a pack, rather than sending
  a message to a page that is gone.

  The runner could not be the receipt: this slice's push, `fd0f2ff`, is run
  `36331380300` -- `Test workspace` and `Lint` dying in three seconds with zero
  steps and `recent account payments have failed or your spending limit needs to
  be increased`, and `Live services` and `Build exe` skipped rather than
  scheduled: the same block as the nine before it. The transcripts above are this
  machine's, run with the flags `ci.yml` uses.
    473 passed; 0 failed  (palantir-desktop, bin)
      4 passed; 0 failed  (palantir-desktop, tests/native.rs)
     31 passed; 0 failed  (palantir-loader, lib)
    224 passed; 0 failed  (palantir-net, lib)
      0 passed; 0 failed; 15 ignored  (palantir-net, tests/live.rs)

$ cargo clippy --workspace --all-targets --locked -- -D clippy::correctness
exit 0, and not one warning in a line this slice added

$ cargo test -p palantir-net --test live --locked -- --ignored --test-threads=1 the_installers_profile
test the_installers_profile_agrees_with_the_mirror_except_for_the_wrapper ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 14 filtered out
```

  `crates/palantir-net/src/engine/forge.rs` is the reader: `installer_url`
  names the installer jar per build (Forge's under the game and the build,
  NeoForge's under the build alone, and none for Fabric and Quilt, whose
  profile is a document), `parse_installer` opens the jar over the engine's
  own cache -- believed for a year, like a version file, because a released
  build's bytes do not move -- and `translate_profile` turns `version.json`
  into the shape `VersionFile` parses, the way `PistonMeta::translated` does
  for Mojang's file. The installer jar needed one dependency this crate did
  not have: `zip` 0.6, the version `palantir-loader` already uses, so the
  lock file gains an edge and no package.

  The measurement is two pinned builds the mirror also serves -- Forge
  `1.21.1-52.1.0` (`recommended`, so the stable default) and NeoForge
  `21.1.172` -- and it says the mirror's copy is a rewrite, the way G95 said
  it about the game's file. Their main classes differ by design (the
  loader's own `ForgeBootstrap`/`BootstrapLauncher` against the mirror's
  ForgeWrapper), their library lists differ by exactly one artifact (Forge:
  the loader's own `:client` against the wrapper; NeoForge: the three
  `org.apache.logging.log4j` jars the wrapper replaces), and the translated
  game arguments are the tail of the mirror's string -- with Forge's three
  `--fml.*` flags appended by the mirror, asserted as the documented suffix
  rather than assumed. `has_order` and the `requires` naming the game are
  the mirror's additions, asserted absent here and present there.

  Two things the era decided. Old Forge builds carry the profile as
  `versionInfo` inside `install_profile.json` with no `version.json`, and
  those are read from that key with their own `minecraftArguments` string
  kept; older still -- neither key -- is refused by name, because a profile
  of that age names tweakers this launcher does not run. And
  `arguments.jvm` is dropped in translation for the reason the mirror drops
  it: its tokens (`${classpath_separator}`, a module path) are ones no
  launch fills, and the wrapper rebuilds that path at launch instead.

  The runner could not be the receipt: billing is still blocked, so pushes
  land as four-second runs with zero steps and the same annotation. The
  transcript above is this machine's, run with the flags `ci.yml` uses.

- [x] G101: the panel's news feed, and the opener its links needed
  CHECK: cargo test --workspace --all-targets --locked
         cargo clippy --workspace --all-targets --locked -- -D clippy::correctness
  EXPECT: test result: ok. 928 passed; 0 failed; 16 ignored, between the seven suites
          (the merged tree's count: G99's Forge commit is under this one)
          exit 0 for clippy, with no warning in a line this slice added
  EVIDENCE: the transcripts of these commands on this tree, and of the live feed the
             section is drawn from:
- [x] G100: the installers' processors patch the client and unpack the maven
      artifacts at install time, the way the official installer does
  CHECK: cargo test --workspace --all-targets --locked
         cargo clippy --workspace --all-targets --locked -- -D clippy::correctness
         cargo test -p palantir-net --test live --locked -- --ignored --test-threads=1 forge_and_neoforge_processors
  EXPECT: test result: ok. 928 passed; 0 failed; 16 ignored, between the seven suites
          exit 0 for clippy, with no warning in a line this slice added
          1 passed, and it installed one Forge build and one NeoForge build for real
  EVIDENCE: the transcripts of these commands on this tree:

```
$ cargo test --workspace --all-targets --locked
    177 passed; 0 failed  (palantir-core, lib)
      8 passed; 0 failed  (palantir-core, tests/compat.rs)
    481 passed; 0 failed  (palantir-desktop, bin)
      4 passed; 0 failed  (palantir-desktop, tests/native.rs)
     31 passed; 0 failed  (palantir-loader, lib)
    227 passed; 0 failed  (palantir-net, lib)
      0 passed; 0 failed; 16 ignored  (palantir-net, tests/live.rs)

$ cargo clippy --workspace --all-targets --locked -- -D clippy::correctness
exit 0; 42 warnings between the crates, the same count as the slice before --
`palantir-core` (lib) 9, its `compat` test 1 and its lib test 10; `palantir-net` 1
and 1 duplicate; `palantir-desktop` (bin) 3 and (bin test) 21 -- none of them in a
line this slice added (`grep news` and `grep open.rs` over the log find no
warning), and `grep -cE "never (used|read|constructed)"` is 0.

$ python - <<'PY'          # the feed, read live
PY
top-level keys: ['articles']
articles: 45
article keys: ['date', 'link', 'summary', 'thumbnail', 'title']
  title: Sync settings across instances
  summary: Keep game options, servers, resource packs, and more the same across your instances.
  thumbnail: https://modrinth.com/news/article/sync-settings/thumbnail.webp
  date: 2026-09-07T19:00:00.000Z
  link: https://modrinth.com/news/article/sync-settings
```

  The panel's second section, and the first thing in this launcher that opens a
  link. `App.vue` draws it from `https://modrinth.com/news/feed/articles.json`:
  the reference's own heading (`app.news.title`), `articles.slice(0, 4)` as
  `NewsArticleCard`s, and a `ButtonLink` -- this launcher's icon-and-label button
  -- to `https://modrinth.com/news`. The feed is the one Modrinth document the
  engine reads that is **not** the API: not under `/v2`, not a project's metadata,
  a publisher's own JSON for its news page. Measured rather than assumed -- 45
  articles, five fields, the names above; a fixture would have agreed with wrong
  keys, which is why the live test is the one that reads it off the service and
  the unit test only proves the reader against a body in that shape.

  **The date, and the cache.** The reference formats the article's date with
  `dateStyle: 'long'`, which is `September 7, 2026`: a twelve-name table and the
  ISO-8601 string's own first ten characters, rather than a date crate that can
  format anything for the sake of one format. A date that cannot be read is drawn
  as it was published rather than as nothing, because a wrong-looking date is a bug
  report and a missing one is silence. The feed is believed for the *project* TTL
  and not the search TTL: a feed of announcements is not a question about what
  people are using right now, and an article half an hour old is still the article
  under the reader's nose.

  **The opener, and why this slice had to grow one.** Two of the section's three
  controls are links -- each card is its own link (`AutoLink`), and *View all news*
  is `NEWS_PAGE_URL` -- so without an opener the section would be drawn out of
  controls that do nothing, which is the one thing this shell's conventions refuse.
  `open.rs` is that opener, and it is the only place this launcher starts a program
  that is neither Minecraft nor Java, so its rules are stated there rather than at
  each call site. Two schemes and not every scheme: `http` and `https`, each
  followed by `//` (a link names a host), everything else refused with a sentence --
  because the feed is a stranger's JSON, and `file:///C:/Windows/System32/cmd.exe`
  or a Windows `cmd:`-style scheme arriving as a string must not become a program.
  The command is a *value*, built per platform by `command_for(platform, url)`, so
  the tests assert all three commands on whichever machine runs them -- and the
  Windows one is `cmd /C start "" <url>` with the empty argument, because `start`
  reads its first quoted argument as the window *title*, so a URL with a space in it
  would otherwise open an empty window named by half of the address. Nothing waits
  for the browser, and a failure comes back as a sentence rather than as a silent
  nothing.

  **What the section does not draw, and each reason.** A card with no title is
  dropped, and so is one whose link this launcher will not open: the panel's job is
  to draw the articles it can hand to the operating system, and `news_shown` is the
  function that says which those are, separating the choice from the drawing so the
  choice is what a test asserts. The **thumbnail is not drawn at all**: the card's
  first element is its image, and nothing in this launcher fetches a picture yet,
  so the title, the summary and the date are drawn and the picture is named as
  missing rather than faked with an empty frame. And an empty or failed feed draws
  **nothing** -- the reference's own `v-if="news.length"` -- rather than a heading
  over no articles; a feed that did not arrive is not a section, and the shell keeps
  the failure in its state for the record.

  Eight tests, none of them about pixels: the live test parses the real feed into
  articles a card can be built from; the engine's proves five defaulted fields, a
  missing summary costing a paragraph rather than the parse, and a feed inside its
  belief costing nothing; `modrinth`'s proves the date label and the fallback; and
  `open`'s three prove the scheme list (including the refusals), the three platform
  commands, and a refused link saying so instead of starting something. The shell's
  two draw a feed of four plus the button and assert the drops -- the untitled
  article, the `file:///` one -- in every theme.

  Why G101 and not G99. The Forge and NeoForge work order
  (`.scratch/HANDOFF-forge-neoforge.md`) names its gate numbers before this slice
  landed, so that two agents writing into one ledger cannot collide; that work put
  its two loaders in one entry and took G99, and this slice -- which is stage 3's
  and has nothing to do with it -- takes the number above it. The two commits
  rebased against each other on the way in, and the counts above are the merged
  tree's, not either slice's alone.

  The runner could not be the receipt: this slice's push, `35b1aeb`, is run
  `36335734138` -- four seconds, `Test workspace` and `Lint` failing with **zero
  steps** and `The job was not started because recent account payments have failed
  or your spending limit needs to be increased`, with `Build exe` and `Live
  services` skipped rather than scheduled: the same block as the eleven before it.
  The transcripts above are this   machine's, run with the flags `ci.yml` uses.
    476 passed; 0 failed  (palantir-desktop, bin)
      4 passed; 0 failed  (palantir-desktop, tests/native.rs)
     31 passed; 0 failed  (palantir-loader, lib)
    232 passed; 0 failed  (palantir-net, lib)
      0 passed; 0 failed; 16 ignored  (palantir-net, tests/live.rs)

$ cargo clippy --workspace --all-targets --locked -- -D clippy::correctness
exit 0; `palantir-core` (lib) 9 warnings, its `compat` test 1 and its lib test
10; `palantir-net` (lib) 1 and its lib test 1 duplicate; `palantir-desktop` (bin)
3 and (bin test) 21 -- none of them in a line this slice added, and
`grep -cE "never (used|read|constructed)"` is 0

$ cargo test -p palantir-net --test live --locked -- --ignored --test-threads=1 forge_and_neoforge_processors
test forge_and_neoforge_processors_install_a_client ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 15 filtered out; finished in 218.20s
```

  `install` in `crates/palantir-net/src/engine/forge.rs` is the official
  installer's sequence in two phases, like the official installer: first
  every library, tool and input the running processors name is resolved
  through the maven roots (the loader's own maven, then central, then
  Mojang's libraries) and digest-checked into the content store against the
  `.sha1` sidecar, then each processor's arguments are expanded from `data`
  (`{MINECRAFT_JAR}`, `{ROOT}`, `{INSTALLER}`, `{LIBRARY_DIR}` and the
  `[maven]` entries, plus `{SIDE}` and `{MINECRAFT_VERSION}`) and run under a
  runtime `palantir-core`'s Java locator found, with the instance root as its
  working directory. The tool is `java -cp` over the resolved jars at the
  `Main-Class` the jar's own manifest declares. Server-only processors are
  skipped on a client install; an output already present and matching is the
  resume case; a processor that fails takes the install with it with its
  index, its coordinate and the tool's own output attached.

  The client jar the processors read is Mojang's, fetched and digest-checked
  through `piston.rs` and the content store -- the live test resolves it the
  way a launch does and hands the stored file over, so no second download of
  anything else exists to be wrong about.

  Three things the live run taught, all about telling inputs apart. The
  installer jar itself moved onto the content store beside the tools: Forge's
  maven stalls a single slow connection -- a 6MB installer outlasts one
  request timeout -- and the store's staging file is where the next attempt
  continues, which the metadata cache cannot do. A product of the chain is
  never resolved: it exists on no maven, so the declared outputs are
  collected first (a server-skipped processor's outputs excluded, since they
  are never written on this side) and anything resolving into them is left
  for the tool that writes them. And a data entry no maven hosts is
  tolerated rather than refused in phase zero -- MCP_DATA extracts its
  mappings out of the neoform zip and DOWNLOAD_MOJMAPS fetches Mojang's own,
  so the run, with the processor named, is what judges those, not the
  resolver. Measured: Forge `1.21.1-52.1.0` runs 3 processors and skips 4,
  NeoForge `21.1.172` runs 6 and skips 4, both patched clients land with the
  digest their last processor declared, and the mirror's file for the same
  versions is still the ForgeWrapper launch those products are for.

  G99's `profile` grew a content-store argument with the installer move, and
  the G99 live test moves with it; its assertions are unchanged.

  The runner could not be the receipt: this slice's push is the run the next
  `gh run list` names, expected four seconds with zero steps and the same
  billing annotation as G99's push (`36333913779`, `Test workspace` and
  `Lint` failing, `Live services` and `Build exe` skipped). The transcript
  above is this machine's, run with the flags `ci.yml` uses.

- [x] G102: the panel's getting-started checklist, and the friends sentence beside it
  CHECK: cargo test --workspace --all-targets --locked
         cargo clippy --workspace --all-targets --locked -- -D clippy::correctness
  EXPECT: test result: ok. 937 passed; 0 failed; 16 ignored, between the seven suites
          exit 0 for clippy, with no warning in a line this slice added
  EVIDENCE: the transcripts of these commands on this tree:

```
$ cargo test --workspace --all-targets --locked
    177 passed; 0 failed  (palantir-core, lib)
      8 passed; 0 failed  (palantir-core, tests/compat.rs)
    490 passed; 0 failed  (palantir-desktop, bin)
      4 passed; 0 failed  (palantir-desktop, tests/native.rs)
     31 passed; 0 failed  (palantir-loader, lib)
    227 passed; 0 failed  (palantir-net, lib)
      0 passed; 0 failed; 16 ignored  (palantir-net, tests/live.rs)

$ cargo clippy --workspace --all-targets --locked -- -D clippy::correctness
exit 0; 42 warnings between the crates, the same count as the slice before --
`palantir-core` (lib) 9, its `compat` test 1 and its lib test 10; `palantir-net` 1
and 1 duplicate; `palantir-desktop` (bin) 3 and (bin test) 21 -- none of them in a
line this slice added, and `grep -cE "never (used|read|constructed)"` is 0. The
count was 43 on the first run of this tree: `checklist_step`'s radio-glyph arm
converted an `Element` into the same type, and clippy's `useless_conversion` was
right, so the arm is what changed rather than the count.
```

  The panel's third section, and the first one whose data is not a service's
  answer. `onboarding-checklist/index.vue` draws it and
  `providers/onboarding-checklist.ts` is where its flags come from: a Tauri plugin
  (`plugin:onboarding-checklist`) whose Rust is not vendored beside this tree, so
  what could be read is the *shape* of that answer and the rules the frontend
  applies to it. Both are written down in `checklist.rs` rather than guessed at in
  the shell -- the generated `OnboardingChecklist` type is four flags, and
  `App.vue`'s own line is `showFriendsList = !showChecklist ||
  hasLoggedIntoModrinth`. Two of the three steps are facts this launcher already
  holds: an instance exists, and an account is signed in. The third is Modrinth's
  and is `false`, because this launcher has no Modrinth sign-in at all; the step is
  drawn outstanding rather than quietly ticked, and its press says the flow is not
  built.

  **The one rule this tree cannot measure, and the reading it uses instead.**
  `show_checklist` is the plugin's and the vendored frontend only *reads* it:
  nothing in it dismisses the checklist, and the provider's own fold is an `&&`
  over the events, so the frontend never decides it either. What this launcher
  reads is completion of the steps it can *finish* -- an instance is missing or no
  account is signed in -- and the third step is out of that reading on purpose. A
  section pinned on a step nobody here can complete would stay up forever, and what
  that step carries is not lost by leaving it out: the prompt moves to the friends
  section beside it, which is exactly where the reference draws the same sentence
  for a reader with no Modrinth session. While the section *is* up the third step is
  drawn with the other two. This is an inference with the reference's alternative
  named beside it, not a measurement, and it is the one place in this slice where
  the answer came from judgement rather than from a file.

  **A debt from G83 is paid.** *Playing as* is drawn `v-show="hasLoggedIntoMinecraft"`
  in the reference, and G83 drew it always and said so, because that flag is the
  checklist's own and the checklist did not exist yet. It is the checklist's second
  fact now, so the card is drawn when there is an account and not otherwise: a
  launcher with no account draws the steps that lead to one, and the empty card is
  what an empty *store* draws. The gate keeps the two states apart in one test.

  **The friends section, in the state this launcher can be in.** `FriendsList.vue`
  behind `v-show="showFriendsList"`: with no credentials it holds one sentence --
  `friends.sign-in-to-add-friends`, whose `<link>` slot is the sign-in and whose
  rest says what it is for -- and no heading, because that component's "Friends"
  heading is inside its own `v-if="userCredentials"`. The generated table keeps the
  markup verbatim, which it has to, since the tag's *name* is the slot a component
  fills; so `text::tagged` is new in this slice, three parts and four tests, and the
  panel draws the sentence with its tags taken out. One departure, the toolkit's:
  iced 0.12's `text` is a single run and there is no `rich_text` widget, so the
  link's words cannot be in the accent while the sentence around them is not. The
  sentence is one pressable paragraph instead of a sentence with a link inside it --
  the same press over a wider target rather than a second control -- and its press
  is the checklist's third step, so the two say the same thing about the same flow.

  Nine tests: four in `checklist.rs` (the reference's own copy and order, the two
  local facts, the third step's move to the friends section, and the friends rule
  read from both sides), one in `text.rs` for the tag split, and four in the shell
  (the facts and the card's gate, the accordion open by default, the three presses,
  and the friends sentence with its own press).

  Still absent, and named rather than drawn empty: the fundraiser banner, whose
  campaign is served by an endpoint this launcher has not been given, and the
  friends list's *signed-in* half, which is Modrinth's authenticated friends API.
  Both need the sign-in flow, which is the same gap the checklist's third step says
  out loud.

  The runner could not be the receipt: this slice's push, `49be552`, is run
  `36339693983` -- the same block as the twelve before it, `Lint` and `Test
  workspace` failing with **zero steps** and `The job was not started because
  recent account payments have failed or your spending limit needs to be
  increased`, with `Live services` and `Build exe` skipped rather than scheduled.
  The transcripts above are this machine's, run with the flags `ci.yml` uses.

- [x] G103: Home draws the reference's welcome screen as the whole page
  CHECK: cargo test --workspace --all-targets --locked
         cargo clippy --workspace --all-targets --locked -- -D clippy::correctness
  EXPECT: test result: ok. 939 passed; 0 failed; 16 ignored, between the seven suites
          exit 0 for clippy, with no warning in a line this slice added
  EVIDENCE: the transcripts of these commands on this tree:

```
$ cargo test --workspace --all-targets --locked
    177 passed; 0 failed  (palantir-core, lib)
      8 passed; 0 failed  (palantir-core, tests/compat.rs)
    492 passed; 0 failed  (palantir-desktop, bin)
      4 passed; 0 failed  (palantir-desktop, tests/native.rs)
     31 passed; 0 failed  (palantir-loader, lib)
    227 passed; 0 failed  (palantir-net, lib)
      0 passed; 0 failed; 16 ignored  (palantir-net, tests/live.rs)

$ cargo clippy --workspace --all-targets --locked -- -D clippy::correctness
exit 0; 42 warnings between the crates, the same count and the same list as the
slice before -- `palantir-core` (lib) 9, its `compat` test 1 and its lib test 10;
`palantir-net` 1 and 1 duplicate; `palantir-desktop` (bin) 3 and (bin test) 21 --
none of them in a line this slice added, and
`grep -cE "never (used|read|constructed)"` is 0. The list *is* the slice
before's: `grep -E "^warning: "` over this run's clippy log and over G102's,
sorted and diffed, are the same 42 lines apart from which compilation unit the
one duplicate is attributed to, which moves between runs.
```

  The bin suite is one test shorter than G102's 493 because the commit before this
  one deleted `launch.rs`'s `#[cfg(test)]` `open_url` and the one test that called
  it: that test really spawned `cmd /C start` with `not-a-url`, and every run of
  the suite popped Windows' "cannot find 'not-a-url'" dialog over the desktop.

  Home's first state, and it is the whole page rather than a card in an empty
  library. `Index.vue` draws `WelcomeScreen` `v-if="isReady &&
  !hasCreatedInstance"` and the library `v-else-if="isReady"`; what Home drew
  before this was the same component's copy inside a card, in the middle of a page
  that still had a toolbar and a search field over it. The page is now the
  reference's own arrangement -- `flex flex-col min-h-full px-6 pb-6 pt-16`, a
  hero centred in what is left and the import block at the foot, the hero's icon
  at `size-[6.25rem]`, the title at `text-2xl font-semibold` (which is
  `page::title`'s own size) over the description at `text-base`, and a `w-72`
  column of the create button and the hint -- and the gate is one place rather
  than two: `Load::Empty` and `Ready(vec![])` are the same answer, because the
  store's own `reload` says an empty library is `Empty`. That one place is
  `pages::home::first_run`, and the shell's subscription reads it too, so the page
  the reader is on and the key that is listened for cannot come apart.

  **Three departures, each written down where it is drawn.** The hero's icon is
  this launcher's own art: the reference's is
  `assets/welcome/modrinth-social-icon.png` and the vendored `assets/` holds
  `branding/` and `external/` and no `welcome/`, so the picture is not in this tree
  and the logo the rail already draws is drawn here rather than a borrowed one
  standing in for it. The **dot pattern** behind the hero is not drawn: it is an
  absolutely-positioned decorative block the reference puts *behind* the icon and
  the title, and iced 0.12 has no overlay widget -- a `Column` places its children
  one after another -- so a pattern here could only be above or below the hero.
  And **neither button is ever disabled**: the reference draws both
  `:disabled="offline"` from `navigator.onLine`, and this launcher has no online
  signal anywhere; what an offline reader meets instead is the flow's own failure,
  said where the flow asks for something over the network.

  **A bug the layout work surfaced.** The hint is
  `Press <shortcut>N</shortcut> to quick create an instance`, the generated table
  keeps that markup verbatim -- it has to, the tag's *name* is the slot a component
  fills -- and this page drew the string whole, so the first reader of the first
  run saw the tags. It is split by G102's `text::tagged` now and the slot is the
  reference's own chip: `h-5 min-w-5 rounded-md border-surface-5 bg-button-bg px-1
  text-xs`, the radius from the theme's `--radius-md` (0.75rem, which is what
  Tailwind's `rounded-md` resolves to in the reference's own sheet).

  **And the key it names is real now.** `WelcomeScreen.vue` listens on the window
  for `n`: lower-cased before comparing, with no `metaKey`, `ctrlKey` or `altKey`
  -- Shift is deliberately not one of its guards, so `Shift+N` opens the creation
  flow there too -- while standing down for an event that came from an input and
  for `navigator.onLine` being false. iced 0.12's
  `iced::keyboard::on_key_press` hands a subscription the key and the modifiers
  and nothing about who had the focus, so the focus guard is the *screen* instead:
  the subscription exists only while the welcome screen is up, which is the one
  screen in this shell with no text field on it, and only while no dialog is over
  it -- which is also where every text field this shell can draw over Home lives.
  That is also what makes the repeat harmless: a held key's first press opens the
  dialog, and the subscription is gone before the second arrives.

  Three tests, and one of them is the guard itself: the key table (`n` and `N`,
  the three modifiers that stop it, and keys that are not the letter), the screen's
  own gate read through `view` on a scratch root that answers `Empty`, and the
  hint's split. The others are the page's existing gates, which now run over the
  hero in all four themes.

  The runner could not be the receipt: this slice's push, `0c19994` -- which also
  carried `5462dfc`, the commit that deleted the window-opening test shim -- is run
  `36430312498`, the same block as the thirteen before it: `Lint` and `Test
  workspace` failed in 5 s with **zero steps** and `The job was not started
  because recent account payments have failed or your spending limit needs to be
  increased`, with `Live services` and `Build exe` skipped rather than scheduled.
  The transcripts above are this machine's, run with the flags `ci.yml` uses.

- [x] G104: the Skins page draws the account's own skins, read from Minecraft's
      own service
  CHECK: cargo test --workspace --all-targets --locked
         cargo clippy --workspace --all-targets --locked -- -D clippy::correctness
  EXPECT: test result: ok. 958 passed; 0 failed; 17 ignored, between the seven suites
          exit 0 for clippy, with no warning in a line this slice added
  EVIDENCE: the transcripts of these commands on this tree -- the *merged* one,
            after G99's and G100's commits landed under this slice. Their own
            entries above carry the counts of the trees they were measured on
            (928 passed with 16 ignored); this slice adds three document tests to
            `palantir-net`'s suite and eight to the desktop's, and G100's own live
            test is the seventeenth `#[ignore]`:

```
$ cargo test --workspace --all-targets --locked
    177 passed; 0 failed  (palantir-core, lib)
      8 passed; 0 failed  (palantir-core, tests/compat.rs)
    500 passed; 0 failed  (palantir-desktop, bin)
      4 passed; 0 failed  (palantir-desktop, tests/native.rs)
     31 passed; 0 failed  (palantir-loader, lib)
    238 passed; 0 failed  (palantir-net, lib)
      0 passed; 0 failed; 17 ignored  (palantir-net, tests/live.rs)

$ cargo clippy --workspace --all-targets --locked -- -D clippy::correctness
exit 0; 42 warnings between the crates, the same count and the same list as the
slice before -- `grep -E "^warning: "` over the two runs, sorted and diffed, are
the same 42 lines apart from which compilation unit the one duplicate is
attributed to -- none of them in a line this slice added, and
`grep -cE "never (used|read|constructed)"` is 0. The first clippy run of this
slice was 43: `clippy::type_complexity` was right about a test fixture written as
an array of nested tuples, which is now a struct called `Paint`.
```

  The Skins page was the largest surface in this stage whose data is *not* a
  Modrinth answer, and the measurement that says so is the reason it could be
  built at all. `pages/Skins.vue` draws through `plugin:minecraft-skins`, whose
  Rust is not vendored here; what the plugin wraps is Minecraft's own document,
  `api.minecraftservices.com/minecraft/profile` -- the same URL `palantir-net`'s
  sign-in already reads for the account's uuid and name -- and that document lists
  the skins and capes the account owns with the `ACTIVE` one marked. So the page is
  drawn from a publisher's own service, and the two lists are only as far away as
  the token the launcher already holds.

  **The picture is this launcher's own arithmetic, and what that costs is written
  where it is drawn.** The reference renders a lit 3D model that turns; iced has an
  `image` widget, so `crate::skin` cuts the texture's documented layout into a front
  view -- the head's 8x8 front at `(8, 8)`, the body's 8x12 at `(20, 20)`, the arms
  at `(44, 20)` and `(36, 52)`, the legs at `(4, 20)` and `(20, 52)` -- and lays the
  six parts beside each other as a paper doll. A 64x32 legacy texture is read by
  mirroring the right limbs into the left slots, which is what Minecraft itself does
  with one arm's and one leg's worth of pixels, and the gate checks the *flip* rather
  than the colour: the right arm's last column is the left arm's first. Three
  departures are named in the module -- no rotation or lighting, no second layer
  (hat, jacket, sleeves, trousers), four-pixel arms for both variants -- and the
  fourth is the page's own: a skin is listed by its `variant` because the document
  names each one by id and nothing else, while the reference's names come from the
  bundles it ships.

  **The seam is the project page's, one variant wider.** `Ask::Skins` carries a
  round and nothing else: which account is the one a launch would sign in as, and
  the token it carries, are the shell's own -- a page has never seen an account file
  -- and an offline account is answered with a sentence rather than a request,
  because there is no skins service for one to ask. The page owes the request as it
  opens (`opening`), so a window opened straight on `/skins` draws the reader's own
  skin rather than an empty gallery, and the round travelling with the answer is what
  drops a reply to a question the page has replaced.

  **One thing is deliberately not drawn.** Applying a skin is a write to the
  reader's own Minecraft account, and it is not this slice: the button says the flow
  is not built rather than pretending, which is the same rule the checklist's third
  step follows. The sections above the account's own lists are still Modrinth's
  bundles and still say so; `NEXT_STEPS.md`'s bullet names all three of the parts
  that are left.

  **What the gates do and do not say.** The document's own half is gated against a
  scripted transport (`the_profile_document_says_which_skin_the_account_is_wearing`,
  `an_account_with_nothing_to_wear_says_so_rather_than_the_first_skin`,
  `a_profileless_account_has_nothing_to_wear_either`), the cut is gated against real
  PNGs built pixel by pixel in the test, and the appearance's own seam
  (`skin::Appearance::of`) is gated for both ways a picture can be missing. What is
  *not* gated is the texture's fetch over the engine -- it needs a live service -- and
  no gate here has seen a real account: everything above is a fixture's answer, which
  is the same limit the live tests exist for.

  **The count moved with the rebase, and the transcript above is the merged tree's.**
  G122 was first measured on a tree whose `palantir-net` suite ran 247 tests with 17
  ignored, which is the 1004 this entry carried; the commits it was rebased onto --
  G112-G114, the engine's own error path keeping the service's sentence, and the two
  mirror assertions -- add four of that crate's tests and one live case, so the
  numbers above are 251, 18 and **1008**. Clippy is the same tree either way: exit 0
  at 42 warnings, the baseline this branch has carried since the locales slice.

  The runner could not be the receipt either: this slice's push, `11bce05` -- which
  also carried `ad8249c`, the slice itself -- is run `36437508333`, the same block as
  the fourteen before it: `Lint` and `Test workspace` failed in 6 s with **zero
  steps** and `The job was not started because recent account payments have failed or
  your spending limit needs to be increased`, with `Live services` and `Build exe`
  skipped rather than scheduled. The transcripts above are this machine's, run with
  the flags `ci.yml` uses.

- [x] G105: the four surfaces stage 3 left are measured -- two need a Modrinth
  session, one is only unfinished, and the plan's last stage-3 item is decided
  rather than implied
  CHECK: grep -rho "invoke('plugin:[a-z-]*" vendor/modrinth-app/app-frontend/src vendor/modrinth-app/ui/src | sed "s/invoke('//" | sort -u | wc -l
         ls vendor/modrinth-app/app-frontend/src-tauri
         grep -rn "billing_internal\|campaign_internal" vendor/modrinth-app/app-frontend/src
         grep -n "invoke('plugin:minecraft-skins" vendor/modrinth-app/app-frontend/src/helpers/skins.ts
         python tools/progress.py --check
         python tools/dashboard.py --check
  EXPECT: 21 plugin namespaces, the four below among them, and no `src-tauri`
          directory at all -- so none of the Rust behind those calls is in this
          tree; exactly the two `_internal` Labrinth routes named below; the
          twelve `plugin:minecraft-skins` calls, of which the writing seven are
          the ones left; both document tools exit 0, with the two open stage-3
          items still open and saying why
  EVIDENCE: the transcripts of these commands on this tree:

```
$ grep -rho "invoke('plugin:[a-z-]*" vendor/modrinth-app/app-frontend/src vendor/modrinth-app/ui/src | sed "s/invoke('//" | sort -u | wc -l
21
$ grep -rho "invoke('plugin:[a-z-]*" vendor/modrinth-app/app-frontend/src vendor/modrinth-app/ui/src | sed "s/invoke('//" | sort -u | grep -E "skins|friends|users|mr-auth"
plugin:friends
plugin:minecraft-skins
plugin:mr-auth
plugin:users
$ ls vendor/modrinth-app/app-frontend/src-tauri
ls: cannot access 'vendor/modrinth-app/app-frontend/src-tauri': No such file or directory
$ grep -rn "billing_internal\|campaign_internal" vendor/modrinth-app/app-frontend/src
vendor/modrinth-app/app-frontend/src/pages/Servers.vue:26:	queryFn: () => client.labrinth.billing_internal.getProducts(),
vendor/modrinth-app/app-frontend/src/components/ui/PrideFundraiserBanner.vue:15:	queryFn: () => client.labrinth.campaign_internal.getPride26(),
$ grep -n "invoke('plugin:minecraft-skins" vendor/modrinth-app/app-frontend/src/helpers/skins.ts
114:	return invoke('plugin:minecraft-skins|get_available_capes', {})
118:	return invoke('plugin:minecraft-skins|get_available_skins', {})
126:	return await invoke('plugin:minecraft-skins|add_and_equip_custom_skin', {
134:	await invoke('plugin:minecraft-skins|equip_skin', {
140:	await invoke('plugin:minecraft-skins|remove_custom_skin', {
146:	await invoke('plugin:minecraft-skins|set_custom_skin_order', {
158:	return await invoke('plugin:minecraft-skins|save_custom_skin', {
174:	return invoke('plugin:minecraft-skins|normalize_skin_texture', { texture })
178:	await invoke('plugin:minecraft-skins|unequip_skin')
182:	await invoke('plugin:minecraft-skins|flush_pending_skin_change')
186:	await invoke('plugin:minecraft-skins|flush_pending_skin_change_for_profile', {
192:	const data = await invoke('plugin:minecraft-skins|get_dragged_skin_data', { path })
$ python tools/progress.py --check
$ echo $?
0
$ python tools/dashboard.py --check
CONFIRMED: the page carries all 106 gates, 6 stage cards and every subject as written
$ echo $?
0
```

  **No code in this slice, and that is the slice.** Stage 3's ledger has kept four
  items open through G96-G104, and each was described the same way: a service this
  launcher does not read. The plan's own note said they need a decision rather than
  another slice, so this gate is the decision, and the measurement it rests on is
  the paragraph above rather than any behaviour of this tree.

  **What the tree can and cannot answer.** The reference's frontend calls **21**
  plugin namespaces. Four of them are the surfaces in question --
  `plugin:minecraft-skins`, `plugin:friends`, `plugin:mr-auth`, `plugin:users` --
  and `app-frontend/src-tauri` is not vendored, so the Rust that answers any of
  them is absent from this repository and cannot be read here at all. That is the
  whole reason these four were never "one more slice": a plugin call is not an
  HTTP endpoint that could be guessed from the frontend, it is a Rust command
  whose implementation is the artifact. What the frontend *does* show is the
  shape of each answer, and where the answer itself comes from when the plugin is
  only a proxy.

  **Two of the four are behind the app's own Modrinth session.** An instance's
  hosting half and the Servers page are one service: `Servers.vue` asks
  `client.labrinth.billing_internal.getProducts()` for the product list, through
  the client the app injects with the reader's Modrinth token. The panel's
  fundraiser banner is the same word twice: `client.labrinth.campaign_internal.getPride26()`.
  `_internal` is the frontend's own naming for a route outside Modrinth's
  published API, which is why neither appears in the public v2 documentation and
  why guessing a shape for them would be invention rather than measurement. The
  friends list's signed-in half is four `plugin:friends` calls behind
  `plugin:mr-auth`'s seven, and `plugin:users` -- search, profile, projects,
  organizations, preferences -- is Labrinth's user service wearing a plugin name.
  This launcher has a Minecraft sign-in and no Modrinth one, so all three of these
  are gated on a session this tree cannot make.

  **One of the four is only unfinished, and it is the one worth taking.** The
  Skins page's remaining half is `helpers/skins.ts`'s writing side:
  `equip_skin`, `add_and_equip_custom_skin`, `remove_custom_skin`,
  `set_custom_skin_order`, `save_custom_skin`, `unequip_skin` and
  `normalize_skin_texture`. Every one of those is Minecraft's own skin service --
  the document G104 already reads and the token a launch already holds -- plus a
  store of the reader's own choices, which the plugin keeps for itself and any
  launcher would have to keep for itself too (`source: 'custom' |
  'custom_external'` against Mojang's `default`). So the gap there is a slice,
  not an account. The same file hands over one measurement cheaply: `determineModelType`
  answers slim-versus-classic from the arm column at (54, 20) -- x 54..55, y
  20..31, opaque pixel or not -- which is a test `skin.rs`'s cutter is already
  holding the pixels for. It is named here so the slice that takes the writing
  half does not have to find it again.

  **What this gate deliberately does not do.** It does not tick a stage-3 item
  off: `tools/progress.py`'s two open entries stand, with their reasons rewritten
  to say which of the four is reachable, and stage 3 stays at its 13 of 17. It
  does not build the Modrinth OAuth path either, though that is the one move that
  would unlock three surfaces at once, because it is a slice of its own size
  (a device or web flow, a token store, and then Labrinth behind it) and it is
  written into `NEXT_STEPS.md` as the alternative so a later session picks
  between the two rather than re-measuring. And it does not write to anyone's
  Minecraft account, which is what the skins writing half does and why that half
  is a slice rather than a line here.

  **What the gates do and do not say.** Nothing in this slice is a test: its
  receipt is the transcripts above, which are measurements of the *reference*,
  plus the two document tools, which fail if the ledger and the plan disagree.
  It asserts no behaviour of this tree and changes none -- no source file is
  touched -- so the suite's last numbers (G104's 958 passed, 0 failed, 17 ignored)
  are still the suite's numbers.

  The runner could not be the receipt either: this slice's push, `2da9f1e`, is run
  `36438186250`, which is the same block as the fourteen before it -- `Test
  workspace` and `Lint` failed in 3 s and 2 s with **zero steps**, and `The job was
  not started because recent account payments have failed or your spending limit
  needs to be increased`, with `Live services` and `Build exe` skipped rather than
  scheduled.

- [x] G106: the Skins page's writing half -- a row's Apply puts one of the
  account's own skins or capes on, through Minecraft's own skin service
  CHECK: cargo test --workspace --all-targets --locked
         cargo clippy --workspace --all-targets --locked -- -D clippy::correctness
         cargo test -p palantir-net --lib --locked wearing_a_skin_posts_the_variant_the_service_asks_for_and_the_texture_it_is
         cargo test -p palantir-desktop --locked --bin PalantirMC wearing_something_is_one_request_and_the_second_press_is_not_a_second_write
  EXPECT: 967 passed; 0 failed; 17 ignored, between the seven suites
          exit 0 for clippy, with no warning in a line this slice added
          the four named tests pass, each in isolation
  EVIDENCE: the transcripts of these commands on this tree:

```
$ cargo test --workspace --all-targets --locked
    177 passed; 0 failed  (palantir-core, lib)
      8 passed; 0 failed  (palantir-core, tests/compat.rs)
    504 passed; 0 failed  (palantir-desktop, bin)
      4 passed; 0 failed  (palantir-desktop/tests/native.rs)
     31 passed; 0 failed  (palantir-loader, lib)
    243 passed; 0 failed  (palantir-net, lib)
      0 passed; 0 failed; 17 ignored  (palantir-net, tests/live.rs)

$ cargo clippy --workspace --all-targets --locked -- -D clippy::correctness
$ echo $?
0
$ grep -c '^warning: ' .scratch/g106-cl.log
42
$ diff <(grep '^warning: ' .scratch/g104-cl.log | sort) <(grep '^warning: ' .scratch/g106-cl.log | sort) && echo IDENTICAL
IDENTICAL

$ cargo test -p palantir-net --lib --locked wearing_a_skin_posts_the_variant_the_service_asks_for_and_the_texture_it_is
test auth::tests::wearing_a_skin_posts_the_variant_the_service_asks_for_and_the_texture_it_is ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 242 filtered out; finished in 0.00s

$ cargo test -p palantir-desktop --locked --bin PalantirMC wearing_something_is_one_request_and_the_second_press_is_not_a_second_write
test pages::skins::tests::wearing_something_is_one_request_and_the_second_press_is_not_a_second_write ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 503 filtered out; finished in 0.00s
```

  **What this slice is, and where it came from.** G105 measured stage 3's four
  remaining surfaces and found exactly one that was *unfinished* rather than out of
  reach: the Skins page's writing half. This is that slice, and it is the last item
  of the plan's own list that can be taken without a Modrinth account.

  **The service's shape, named rather than guessed.** The reference's plugin Rust is
  not in this tree (G105: no `src-tauri`), so what its `plugin:minecraft-skins`
  calls *do* had to be read from the service's own protocol rather than from a
  vendored caller. The four requests are `POST
  api.minecraftservices.com/minecraft/profile/skins` with `{"variant":..,"url":..}`,
  `DELETE …/profile/skins/active`, `PUT …/profile/capes/active` with
  `{"capeId":..}`, and `DELETE …/profile/capes/active` -- cross-checked against two
  independent public implementations of the same API (the wiki.vg-documented shape
  as implemented by `minecraft-launcher-core-node`'s `mojang.ts`, whose
  `setSkin`/`resetSkin`/`showCape`/`hideCape` are these four requests down to the
  verb and the body). The provenance is written here because it is weaker evidence
  than the rest of this file: every other service claim in the ledger quotes a
  vendored call site, and this one cannot.

  **Where the split falls, and why.** `palantir_net::MicrosoftAuth::wear` is one
  entry point over four methods, and `SkinChange` is the type a caller describes the
  change with: which verb, which URL and whether there is a body are the module's
  business, so no caller can misspell `PUT` or forget that the cape endpoint takes
  an id rather than a texture. A change names only things the account already owns,
  which is why no variant of that type *can* upload. Above it, `Store::wear` is the
  same pass-through `Store::skins` is, and the page's own `Wear` carries a round so
  an answer can be matched to the change waiting for it -- the same seam `Ask::Skins`
  uses, one step further out, and the only place a page can ask for a *write*.

  **What the page does with it.** Each row that is not already in force gets the
  reference's own Apply (`AppSkinsApplyButton`), and the cape list gets the
  reference's `AppSkinsModalNoneCapeOption` row to take one off. The rule is a
  function (`wear_skin`/`wear_cape`) rather than a condition inside the drawing, so
  the gate can read it: nothing for a skin already worn, nothing for the cape
  already on, and nothing at all while a change is in flight -- a second press would
  be a second write to the reader's own account. A failure comes back as a sentence
  in the slot every other failure this page has goes; a success comes back as
  **silence plus a reload**, because what changed is Minecraft's document and the
  check moving to the new row is a better confirmation than anything this launcher
  could write about it. The header keeps the reference's Add button and loses its
  Apply: that one acts on the reference's preview panel, which renders a *candidate*
  skin, and this page has no such panel -- the doll draws what is in force -- so an
  Apply there would be a button with nothing to act on. One control dropped with
  its reason recorded, rather than a control that lies.

  **What is deliberately not built.** Three things, named on the page rather than
  approximated. The file upload (`add_and_equip_custom_skin`, `save_custom_skin`,
  `normalize_skin_texture`) is a file dialog and a multipart body, and the plugin's
  local store of the reader's own picks (`source: 'custom' | 'custom_external'`)
  is what the reference's Saved-skins sections are drawn from -- neither is here, and
  the page says so where the button is. `unequip_skin` is *implemented* in the client
  (`SkinChange::NoSkin`) and has no control: the reference reaches it from its edit
  modal, and that modal -- which is also where a skin's arm style is chosen by hand
  -- is not this page. And the sections above the account's lists are still
  Modrinth's bundles, which are service answers G105 found out of reach.

  **What the gates do and do not say.** Everything here is offline: the four
  requests are gated against a scripted transport that now records bodies as well as
  URLs (`MapTransport::bodies`), including the two refusals that matter -- a 401
  reads as *sign in again* rather than as a transport failure, and any other non-2xx
  carries the service's own `errorMessage`, which is what a reader can act on. The
  page's rules, the round-matching and the one-write-at-a-time guard are gated
  directly. What is **not** gated is a real write: exercising it would change the
  appearance of a real account, so no test does it, and nothing here has ever seen
  the service accept one. That is the honest limit of this slice, and it is the same
  limit G104's read carries.

  The runner could not be the receipt either: this slice's push, `5fbc3bf`, is run
  `36440609824`, which is the same block as the fifteen before it -- `Test
  workspace` and `Lint` failed in 6 s with **zero steps** and `The job was not
  started because recent account payments have failed or your spending limit needs
  to be increased`, with `Live services` and `Build exe` skipped rather than
  scheduled.

- [x] G107: what the mirror's Forge and NeoForge profiles actually are -- measured,
  after the ledger claimed it from an inference
  CHECK: grep -rn "mavenFiles\|maven_files" crates/
         sed -n '247,251p' crates/palantir-desktop/src/install.rs
         python tools/progress.py --check
         python tools/dashboard.py --check
  EXPECT: `palantir-core` is the only crate that names the key, and the install
          plan fetches `libraries`, `native_libraries` and `main_jar` -- neither
          list includes `maven_files`
  EVIDENCE: the two documents, read whole (a browser fetch rather than the tree's
            own client, because this is a measurement of somebody else's service):
            `https://meta.prismlauncher.org/v1/net.minecraftforge/66.0.6.json` and
            `https://meta.prismlauncher.org/v1/net.neoforged/21.1.172.json`. The
            lines that matter, verbatim from each:

```
Forge 66.0.6, beside its 40-odd libraries:
  "mainClass": "io.github.zekerzhayard.forgewrapper.installer.Main",
  "mavenFiles": [ { "name": "net.minecraftforge:forge:26.3-66.0.6:installer",
                    "url": ".../forge/26.3-66.0.6/forge-26.3-66.0.6-installer.jar" },
                  { "name": "com.github.jponge:lzma-java:1.3" },
                  { "name": "com.nothome:javaxdelta:2.0.1" },
                  ... ]

NeoForge 21.1.172, beside its 40-odd libraries:
  "mainClass": "io.github.zekerzhayard.forgewrapper.installer.Main",
  "minecraftArguments": "... --fml.neoForgeVersion 21.1.172 --fml.mcVersion 1.21.1
                        --fml.neoFormVersion 20240808.144430 --launchTarget forgeclient",
  "mavenFiles": [ { "name": "net.neoforged:neoforge:21.1.172:installer" },
                  { "name": "net.neoforged:neoform:1.21.1-20240808.144430@zip" },
                  { "name": "net.neoforged.installertools:binarypatcher:2.1.2:fatjar" },
                  ... ]

$ grep -rn "mavenFiles|maven_files" crates/
crates/palantir-core/src/version/profile.rs:50:    pub maven_files: Vec<Library>,
crates/palantir-core/src/version/profile.rs:132:        for maven in &file.maven_files {
crates/palantir-core/src/version/profile.rs:193:        self.maven_files.push(maven.clone());
crates/palantir-core/src/version/profile.rs:479:    fn maven_files_and_agents_skip_natives_and_inactive() {

$ sed -n '247,251p' crates/palantir-desktop/src/install.rs
    for library in profile
        .libraries
        .iter()
        .chain(profile.native_libraries.iter())
        .chain(profile.main_jar.iter())
```

  **What this corrects.** G99's note in `NEXT_STEPS.md` (and the commit that
  carried it) said a Forge instance would resolve "a profile whose client jar was
  never patched", which would break a launch. That was an inference from G99's own
  measurement -- that the two files differ by exactly ForgeWrapper -- and not a
  reading of the mirror's document. The document says something else, and the
  difference matters for whoever takes the flip next.

  **What the mirror's profiles are.** Both name
  `io.github.zekerzhayard.forgewrapper.installer.Main` as their main class and
  carry a second key beside `libraries`: `mavenFiles`, holding the loader's own
  installer jar, the `neoform`/`installertools`/`binarypatcher` tools and the
  launcher stack the wrapper re-launches into. They are **self-installing at first
  launch** -- the wrapper is the installer -- which is exactly why the mirror can
  answer `net.minecraftforge` and `net.neoforged` at all, and why
  `published_loader` answering `None` for those uids is not itself a bug.

  **The finding that matters here.** `install::plan` walks `libraries`,
  `native_libraries` and `main_jar`. It does not walk `maven_files`, and nothing
  outside `palantir-core`'s own merge names that key, so on a mirror-resolved Forge
  or NeoForge instance the installer the wrapper is asked to run is never fetched
  by this launcher. Whether the wrapper then fails or fetches the file itself is
  **not** decidable from this tree -- the wrapper's Rust is not here either (G105:
  no `src-tauri`, and this is a jar rather than a plugin) -- and that is the honest
  edge of this measurement.

  **What it settles.** The order named in `NEXT_STEPS.md` stands, and the *reason*
  is now the stronger one: the flip is not merely the safer order, it is the only
  end state this launcher can finish, because the installer's own translated profile
  needs no wrapper and no `mavenFiles` at all -- its libraries are the real launcher
  stack, and the patched client is the product G100's processors already produce and
  digest-check. A slice that takes it runs the install first, with the patched client
  and its declared digests as the resume test, and flips `published_loader`
  afterwards; the live test that would prove it is the shape G100's own (a real
  Forge and a real NeoForge client, both patched, ~218 s).

  **What the gates say.** This slice is a measurement and a correction: no source
  file changes, and the receipt is the two documents above plus the two document
  tools. It proves nothing about a launch, and it deliberately does not touch the
  routing -- the plan's own note that these two uids need a desktop-track decision
  rather than an engine one still holds.

  The runner could not be the receipt either: this slice's push, `50fd8cc`, is run
  `36441309064`, which is the same block as the sixteen before it -- zero steps and
  `The job was not started because recent account payments have failed or your
  spending limit needs to be increased`, with `Live services` and `Build exe`
  skipped rather than scheduled.

- [x] G120: the reference's own other locales compile into sparse tables, with the
  plural rules measured instead of inferred from English's two
  CHECK: python tools/gen_text.py --check
         python tools/gen_locale.py --check
         python tools/gen_locale.py --report
         cargo test --workspace --all-targets --locked
         cargo clippy --workspace --all-targets --locked -- -D clippy::correctness
  EXPECT: both generators report byte-identical output
          the report lists 33 trees, 89,177 leaves and 2,700,238 bytes of translated text
          970 passed; 0 failed; 17 ignored, between the seven suites
          clippy exit 0 with 42 warnings -- G106's own count, unchanged
  EVIDENCE: the transcripts of these commands on this tree:

```
$ python tools/gen_text.py --check
text generation is byte-identical

$ python tools/gen_locale.py --check
locale generation is byte-identical

$ python tools/gen_locale.py --report
tag      leaves   keys fallback plural select number    # arms
ar-SA      1577   1577     2269     19      5      8   45 few=13, many=13, one=19, other=19, two=13, zero=2
cs-CZ      2554   2554     1292     52      5     22   56 =0=1, few=28, many=3, one=52, other=52
de-CH      3792   3792       54     79     10     30   57 =0=1, one=79, other=79
de-DE      3792   3792       54     80     10     30   58 =0=1, one=80, other=80
en-US      3846   3846        0     80     10     32   56 =0=1, one=80, other=80
fi-FI       515    515     3331      2      0      2    2 one=2, other=2
he-IL      1082   1082     2764     18      0      2    6 one=18, other=18
ja-JP      2989   2989      857     33      8     31   21 =0=1, one=21, other=33
pl-PL      3785   3785       61     81     10     32   73 =0=1, few=56, many=9, one=80, other=81
ru-RU      3753   3753       93     81     10     21  102 =0=1, =1=13, few=60, many=2, one=72, other=81
th-TH       459    459     3387      2      0      1    1 other=2
uk-UA      3772   3772       74     79     10     23  103 =0=1, =1=1, =2=1, few=67, many=38, one=78, other=79
 (all 33 rows printed; the six above are the shapes, and the rest are the same list)

trees            33
leaves           89,177 across 3846 English keys (70.3% translated)
value bytes      2,700,238 of translated text
index bytes      178,354 (a u16 per entry)

arms a language's own rule can never select (dead in the reference too):
  id-ID    one
  ja-JP    one
  ko-KR    one
  vi-VN    one
  zh-CN    one
  zh-TW    one

$ cargo test --workspace --all-targets --locked
    177 passed; 0 failed  (palantir-core, lib)
      8 passed; 0 failed  (palantir-core, tests/compat.rs)
    507 passed; 0 failed  (palantir-desktop, bin)
      4 passed; 0 failed  (palantir-desktop/tests/native.rs)
     31 passed; 0 failed  (palantir-loader, lib)
    243 passed; 0 failed  (palantir-net, lib)
      0 passed; 0 failed; 17 ignored  (palantir-net, tests/live.rs)

$ cargo test -p palantir-desktop --locked --bin PalantirMC locale_gen::
test locale_gen::tests::every_table_is_sorted_so_a_lookup_can_be_a_binary_search ... ok
test locale_gen::tests::no_index_is_past_the_english_table ... ok
test locale_gen::tests::the_tags_are_unique_and_find_agrees_with_the_list ... ok

$ cargo clippy --workspace --all-targets --locked -- -D clippy::correctness
$ echo $?
0
$ grep -c '^warning: ' .scratch/g120-clippy3.log
42
$ grep -c locale_gen .scratch/g120-clippy3.log
0
```

  **The size question, decided by measurement.** The job's first gate was whether
  all 33 tables fit in the binary or only a chosen set, and the answer is a number
  rather than an opinion. Three configurations of `palantir-desktop`, each built
  with `CARGO_INCREMENTAL=0` after `touch src/main.rs` so that a whole crate
  recompile is being timed and not an incremental patch, on this machine:

```
                                crate rebuild   target/debug/PalantirMC.exe
tables absent                        45 s          394,234,505 bytes
tables for 8 locales                 42 s          395,830,026 bytes  (+1,595,521)
tables for all 33                    61 s          398,847,453 bytes  (+4,612,948)

the data itself, from the tool: 2,700,238 value bytes + 178,354 index bytes
                                = 2,878,592 bytes
generated source: 89,448 lines, 4,114,561 bytes
```

  So **all 33 ship**. The marginal cost is 2,878,592 bytes of data, +4,612,948 on a
  debug artifact and roughly +16 s on a crate rebuild, and the two small
  configurations are 42 s and 45 s apart from each other -- which is to say the
  8-locale build is not measurably cheaper than none, and the all-33 build is the
  only one whose time is a real signal. What the measurement buys is a language
  setting that is complete: every locale the reference publishes is in the binary
  and works with no network, where the reference itself 
  `fetchMessages`es the other 32 at runtime and bundles English only. A release
  artifact was not measured -- the debug delta is reported as the debug delta, and
  the 2,878,592 data bytes are exact either way.

  **What a table is.** `tools/gen_text.py` compiles the reference's English;
  `tools/gen_locale.py` compiles the other 32 and **imports** `gen_text` rather
  than copying its ICU parser, because a locale the generator accepts and a locale
  the runtime renders have to agree about what a message means. A table is the
  reference's own sparse shape -- `(position in text_gen::ALL, template)`, sorted
  by position so a lookup is a binary search. The position rather than the key is
  not an optimisation: every locale's keys are a subset of English's, verified in
  the tool and refused if a tree ever grows one that is not (0 extra keys across
  the 33 today), so the key string is already in the binary once, in
  `text_gen::NAMES`. Repeating it would be 2.5 MB of duplicate text. A key that is
  not in a table falls back to English, which is the reference's own
  `fallbackLocale: 'en-US'` in `app-frontend/src/i18n.config.ts`.

  **The plural question, and the two claims this slice corrects.** The generator
  used to refuse any plural category other than English's `one` and `other`.
  This slice's own measurement of the corpus says that refusal was aimed at the
  wrong thing and that the plan's note about it was half right:

  1. **`zero`, `two`, `few` and `many` are real arms in this corpus.** 6 locales
     carry `few` (`ar-SA` 13, `cs-CZ` 28, `pl-PL` 56, `ru-RU` 60, `sr-CS` 23,
     `uk-UA` 67), 5 carry `many`, and `ar-SA` carries `zero` and `two` as well --
     `{count, plural, zero {..} one {..} two {..} few {..} many {..} other {..}}`
     in `app.screenshots.selection.delete-description`. So the tool now compiles a
     locale with **the whole CLDR category set** as legal arms
     (`gen_text.ALL_PLURAL_CATEGORIES`, threaded through the parser this slice
     widens) and refuses only what is genuinely unsupported: an arm key that is
     not a category and not `=N`, and the ICU types and quoting it already
     refused. English's own output is unchanged -- `gen_text.py --check` still
     prints `byte-identical`, which is the receipt for that.
  2. **`ar-SA` is compiled but is not one of the languages the reference offers.**
     The plan and this work order both say "33 locales". There are 33 locale
     *trees* on disk, and `LOCALES`, in `ui/src/composables/i18n.ts`, lists **32**
     codes -- `ar-SA` is present as files and commented out of that list, with the
     comment `Commented out as it's RTL - will enable when we have better RTL
     support`. `buildLocaleMessages` drops any tree whose tag is not in `LOCALES`,
     so the reference cannot render `ar-SA` at all. This slice compiles all 33
     anyway -- the data exists and a table is a measurement -- and the *offer* is
     the reference's 32; that distinction is [`crate::locale`]'s to keep in G121
     and is recorded here so nobody re-derives it. It also matters for G128: the
     plan names `ar-SA` as one of the two locales to lay out right-to-left, and
     the reference's own reason for excluding it is that it has no RTL support.

  **The rules, and the dead arms.** Each table carries the primary language
  subtag, which is what selects a plural rule at runtime (`Intl.PluralRules`,
  which is what vue-i18n's default pluralization is -- `createI18n` passes only
  `messageCompiler`, no `pluralRules`). `CLDR_CATEGORIES` in the tool is the one
  place a rule lives on this side of the fence and is what the report's last block
  is computed from: 6 locales -- `id-ID`, `ja-JP`, `ko-KR`, `vi-VN`, `zh-CN`,
  `zh-TW` -- carry `one` arms that their own language's rule can never select,
  because those languages are `other`-only in CLDR. Those arms are dead in the
  reference too, so they are reported rather than refused: a translation's dead
  arm is upstream's fact, and refusing it would refuse six whole locales to make a
  lint happy.

  **One real bug, found by the correctness lint.** The reference's translations
  contain invisible characters -- `pt-BR` writes `usados<U+200B><U+200B>apenas`,
  `ru-RU` writes `<U+200B><U+200B>` inside a sentence, `he-IL` carries `U+200E`
  marks -- 28 of them across `U+00AD`, `U+200B`, `U+200E` and `U+200F`. Emitted
  raw they failed the build: `clippy::invisible_characters` is a *correctness*
  lint, so `-D clippy::correctness` denied it and clippy exited 101. The fix is in
  the shared `escape`, which now writes any Unicode *format*, *control*, *line* or
  *paragraph* character as a `\u{...}` escape. That is a faithful round trip --
  `\u{200B}` decodes to the same string the locale holds -- and it changes nothing
  about English, whose locale has none of them. Stripping the characters would
  have made the table agree with clippy instead of with the reference, which is
  the wrong way round.

  The runner could not be the receipt either: this slice's push, `22176da`, is run
  `36447139380`, one of the 74 pushes since `36038333030` that have died the same
  way -- `Lint` and `Test workspace` failed in 2 s and 3 s with **zero steps** and
  `The job was not started because recent account payments have failed or your
  spending limit needs to be increased`, with `Live services` and `Build exe`
  skipped rather than scheduled.

- [x] G108: the User profile page becomes real, off Modrinth's *published* API --
  the header's four facts and the whole projects list, with the two halves that
  need a session still named as absent
  CHECK: cargo test --workspace --all-targets --locked
         cargo clippy --workspace --all-targets --locked -- -D clippy::correctness
         grep -c '^warning: ' .scratch/g108-cl.log
         diff <(grep '^warning: ' .scratch/g106-cl.log | sort) \
              <(grep '^warning: ' .scratch/g108-cl.log | sort)
  EXPECT: 983 passed; 0 failed; 17 ignored, between the seven suites
          exit 0 for clippy, with no warning this slice added
          and the transactions these two documents are read by:
          GET /v2/user/{id|username} and GET /v2/user/{id}/projects
  EVIDENCE: the transcripts of these commands on this tree (`.scratch/g108-ws.log`,
            `.scratch/g108-cl.log`):

```
$ cargo test --workspace --all-targets --locked ; echo $?
    177 passed; 0 failed  (palantir-core, lib)
      8 passed; 0 failed  (palantir-core, tests/compat.rs)
    516 passed; 0 failed  (palantir-desktop, bin)
      4 passed; 0 failed  (palantir-desktop/tests/native.rs)
     31 passed; 0 failed  (palantir-loader, lib)
    247 passed; 0 failed  (palantir-net, lib)
      0 passed; 0 failed; 17 ignored  (palantir-net, tests/live.rs)
0

$ cargo clippy --workspace --all-targets --locked -- -D clippy::correctness ; echo $?
0
$ grep -c '^warning: ' .scratch/g108-cl.log
42
$ diff <(grep '^warning: ' .scratch/g106-cl.log | sort) <(grep '^warning: ' .scratch/g108-cl.log | sort)
6,7c6,7
< warning: `palantir-net` (lib test) generated 1 warning (1 duplicate)
< warning: `palantir-net` (lib) generated 1 warning (run `cargo clippy --fix ...)
---
> warning: `palantir-net` (lib test) generated 1 warning (run `cargo clippy --fix ...)
> warning: `palantir-net` (lib) generated 1 warning (1 duplicate)
```

  The only difference against the previous slice's clippy transcript is which of
  the two *summary* lines carries `(1 duplicate)`: the same crate, the same single
  warning, the same 42 lines. The lint itself is untouched, so this slice adds none.

  **What this slice is.** G105 measured stage 3's four remaining surfaces and found
  the profile page's *session-gated* half (collections, organizations) out of reach
  and its `plugin:users|get_user_profile` route -- Labrinth's internal v3 user
  service -- absent from this tree. What it also found is that Modrinth's
  **published** v2 API answers the same account: `GET /v2/user/{id or username}`
  returns `id`, `username`, `name`, `avatar_url`, `bio`, `created` and a role and
  badge mask, and `GET /v2/user/{id}/projects` returns the projects that account
  owns. This slice draws the page from those two documents instead of from a
  placeholder, and the header's four facts are computed the way the reference
  computes them: the project count and the *sum* of the projects' downloads are its
  own `reduce` over the list (`:projects-count="projects.length"`,
  `:downloads="sumDownloads"`), because the API publishes no total.

  **What was measured, and where.** `user/modrinth` is the account whose document is
  quoted in this slice's own fixture: id `2REoufqX`, `"name": null`, created
  `2023-11-13T23:22:36.604990Z`, and `user/2REoufqX/projects` returns `[]` while
  `user/jellysquid3/projects` returns real documents keyed by `id` (a search hit
  keys the same field as `project_id`, which is why `ModrinthUserProject` is its own
  type rather than a second reading of `ModrinthSearchHit`). Two things follow that
  the code now says out loud:

  * **`name` is nullable.** `#[serde(default)]` covers a field that is *absent*; it
    does not cover one that is present and `null`, and the live document writes
    `null` for an account with no display name. `modrinth::null_as_empty` is the
    one-line deserializer for it, used on the two fields the service marks nullable
    (`name` and a project's `icon_url`) and on no others -- defaulting a field that
    the schema says is a string would be this launcher inventing a shape.
  * **`name` and `username` are two strings.** `ModrinthUser::display_name` and
    `has_separate_username` keep both: an account with no display name is drawn
    once, by its handle, instead of twice with an `@` in front of the second.

  **A duplicate type removed rather than added to.** The crate already had a
  `ModrinthUser { username: String }` at `modrinth.rs:364` -- the member list's
  cut-down user -- so this slice's first compile was `error[E0428]: the name
  `ModrinthUser` is defined multiple times`. The fix is not a rename: a member
  list's `user` *is* the whole user document, and every other field of it was being
  dropped on purpose by nobody. The small type is gone and `ModrinthMember.user` is
  the real one, which is a net smaller module and one fewer place a name is parsed
  by a different struct than a profile's.

  **The strip's order is the reference's third order.** A profile's filter tabs are
  built from the types the user actually has (`catalogProjectTypes(projects)`) and
  sorted by `sortProjectTypes`, which reads `PROJECT_TYPE_ORDER` from
  `ui/src/utils/project-types.ts`: mods first, **modpacks fifth**. That is neither
  `ProjectType::TABS`' order nor `ALL`'s, so `ProjectType::PROFILE_ORDER` is a third
  table with the file quoted above it rather than a reuse of a wrong one.

  **The strip's links are plural, and now parse.** `UserProfilePageLayout.vue`
  builds each `href` as `` `${profilePath}/${projectType}s` `` -- `/user/x/mods`, not
  `/user/x/mod`. Before this slice no profile *filter* link existed anywhere in this
  tree, so the address grammar only ever saw the singular; a strip drawn from the
  reference's own spelling would have produced links this launcher's own
  `Address::parse` refuses (a link that does nothing when pressed).
  `ProjectType::profile_token` writes the reference's spelling and
  `from_profile_token` reads **both**, so this launcher's links and an address copied
  out of the reference are the same page. `collections` -- the reference's fourth
  link -- is still refused rather than guessed at, and a test says so.

  **What the page asks, and what it keeps.** The page asks for a profile the way
  Discover and the project page ask for theirs: one `Asked { user, round }` goes out
  of `pages::user::State::opening`, the shell runs `Store::user` off the frame
  thread, and the answer returns as `pages::Message::user_result`. `Store::user` is
  three requests -- the profile by the *name* the address spells, the projects by
  the **id** only that document carries, and the avatar over the engine's pool --
  and the avatar's failure is not the page's: a machine with no connection still has
  a name, a bio and a list, which is the same split `skin::Appearance::of` makes for
  a doll. Selecting a filter is *not* a request: it is a navigation (`Open::User`),
  and `Screen::retarget` follows the address with `State::filter`, so a filter press
  redraws from the answer the page already holds and only a *different user* builds
  a new page.

  **Two smaller corrections the page carried.** The placeholder's refresh button was
  labelled with `app.library.sort.label`, whose message is *"Sort by"* -- a control
  that said it sorted the page it reloads. It is `button.refresh` (*"Refresh"*) now,
  which is the reference's own label for it. And `Screen::view` no longer takes the
  `Address`: the profile page was the last caller that wanted one, for the name in
  its header, and the name now lives in the page's state because it is also what the
  page *asks* by -- `update` is handed no address to ask with.

  **What is still not here, named rather than drawn empty.** Collections and
  organizations (the v3 user service, G105); the reader's-own empty sentence
  (`State::empty_sentence(true)`), which needs a Modrinth account to compare the
  profile's id against and is kept as the reference's copy with its unreachability
  stated in the doc comment; the header's Edit and overflow actions (report, block,
  copy id, copy permalink), all of which act on an account this launcher is not
  signed in to; and the projects' icons, which would be one fetched and cached
  picture per row -- the rows carry every *word* the service publishes (title,
  summary, type tag, downloads, published date) and the page's one picture is the
  avatar, which is what the reference's own web profile treats the same way.

  **The runner could not be the receipt.** This slice's push is run `36452089947`,
  one more in the block G120 counted: `Test workspace` failed in 3 s and `Lint` in
  2 s with **zero steps** and `The job was not started because recent account
  payments have failed or your spending limit needs to be increased`, with `Live
  services` and `Build exe` skipped rather than scheduled. Nothing here was measured
  on the runner; the numbers above are this machine's.

  **And they are this machine's *before* the rebase.** The push had to be rebased
  onto G120's locales slice, and the two suites were not run again after it -- the
  counts above are 983 passed and 42 clippy warnings on the tree as this slice left
  it, not on the merged tree, which carries G120's own tests as well. Re-running
  them is the first thing a later reader of this entry should do; nothing in the
  conflict resolutions was a source file (`GATES.md` and `tools/progress.py` only,
  both merged by hand with both sides kept).

- [x] G109: what a Modrinth session *is* here, measured -- the two published ways in, the
  four surfaces each would reach, and where a token would have to live
  CHECK: grep -rho "invoke('plugin:mr-auth|[a-z_]*'" vendor/modrinth-app/app-frontend/src | sort -u
         grep -n "export type ModrinthCredentials" -A 6 vendor/modrinth-app/app-frontend/src/helpers/mr_auth.ts
         grep -n "siteUrl\|labrinthBaseUrl\|archonBaseUrl" vendor/modrinth-app/app-frontend/src/config.ts
         grep -rho "client\.labrinth\.[a-zA-Z0-9_]*\.[a-zA-Z0-9_]*" vendor/modrinth-app/{app-frontend,ui}/src | sed 's/\.[a-zA-Z0-9_]*$//' | sort -u
         grep -rho "client\.archon\.[a-zA-Z0-9_]*\.[a-zA-Z0-9_]*" vendor/modrinth-app/{app-frontend,ui}/src | sed 's/\.[a-zA-Z0-9_]*$//' | sort -u
         grep -rho "client\.archon\.[a-zA-Z0-9_.]*" vendor/modrinth-app/{app-frontend,ui}/src | sort -u | wc -l
         grep -rn "campaign_internal\|users_v3.getAuthenticated\|friends_v3" vendor/modrinth-app/{app-frontend,ui}/src
         python tools/progress.py --check
         python tools/dashboard.py --check
  EXPECT: seven `plugin:mr-auth` commands and the credential shape they return
          three hosts, and every service namespace each host carries
          16 Labrinth namespaces, 11 Archon ones and 67 distinct Archon methods
          both document tools exit 0, with the two open stage-3 items still open
  EVIDENCE: the transcripts of these commands on this tree, plus the two published
            pages this slice needed and could not read from the tree (they are
            cited by URL and quoted; nothing else here is external):

```
$ grep -rho "invoke('plugin:mr-auth|[a-z_]*'" vendor/modrinth-app/app-frontend/src | sort -u
invoke('plugin:mr-auth|cancel_modrinth_login'
invoke('plugin:mr-auth|get'
invoke('plugin:mr-auth|get_all'
invoke('plugin:mr-auth|logout'
invoke('plugin:mr-auth|modrinth_login'
invoke('plugin:mr-auth|remove_account'
invoke('plugin:mr-auth|set_active'

$ grep -n "export type ModrinthCredentials" -A 6 vendor/modrinth-app/app-frontend/src/helpers/mr_auth.ts
8:export type ModrinthCredentials = {
9-  session: string
10-  expires: string
11-  user_id: string
12-  active: boolean
13-}

$ grep -n "siteUrl\|labrinthBaseUrl\|archonBaseUrl" vendor/modrinth-app/app-frontend/src/config.ts
3: siteUrl            = 'https://modrinth.com'
4: labrinthBaseUrl    = 'https://api.modrinth.com'
7: archonBaseUrl      = 'https://archon.modrinth.com'
10: sharedInstancesBaseUrl = 'https://shared-instances.modrinth.com'

$ ... client.labrinth.<namespace> ... | sort -u
client.labrinth.attribution_internal   client.labrinth.notifications_v2
client.labrinth.billing_internal       client.labrinth.projects_v2
client.labrinth.campaign_internal      client.labrinth.projects_v3
client.labrinth.collections            client.labrinth.users_v2
client.labrinth.content_v3             client.labrinth.users_v3
client.labrinth.external_projects_internal   client.labrinth.versions_v2
client.labrinth.friends_v3             client.labrinth.versions_v3
client.labrinth.images_v3              client.labrinth.moderation_internal

$ ... client.archon.<namespace> ... | sort -u
client.archon.actions_v1        client.archon.options_v1
client.archon.backups_queue_v1  client.archon.properties_v1
client.archon.backups_v1        client.archon.server_users_v1
client.archon.content_v1        client.archon.servers_v0
client.archon.sockets           client.archon.servers_v1
client.archon.sync

$ ... client.archon.<method> ... | sort -u | wc -l
67

$ grep -rn "campaign_internal\|users_v3.getAuthenticated\|friends_v3" vendor/modrinth-app/{app-frontend,ui}/src
app-frontend/src/App.vue:349:   queryFn: () => tauriApiClient.labrinth.users_v3.getAuthenticated(),
app-frontend/src/pages/Skins.vue:272: queryFn: () => client.labrinth.users_v3.getAuthenticated(),
app-frontend/src/components/ui/PrideFundraiserBanner.vue:15:
                                queryFn: () => client.labrinth.campaign_internal.getPride26(),
ui/src/layouts/wrapped/hosting/manage/[id]/access/access.vue:232:
                                queryFn: () => client.labrinth.friends_v3.list(),

app-frontend/src/pages/Servers.vue:26:
                                queryFn: () => client.labrinth.billing_internal.getProducts(),

$ sed -n '557,560p' ui/src/components/servers/ServerListing.vue
        const fsAuth = await archon.servers_v0.getFilesystemAuth(props.server_id)
$ grep -c "archon.sockets.on" ui/src/composables/server-context-runtime.ts
5
```

  The two published pages, which are the only evidence here that is not the vendored
  tree, and which this slice read because the tree cannot answer them:

  * `https://docs.modrinth.com/api/` -- *"This API has two options for
    authentication: personal access tokens and OAuth2. All tokens are tied to a
    Modrinth user and use the Authorization header"*, with the header's shape
    spelled out (`Authorization: mrp_RNtLRSPmGj2pd1v1ubi52nX7TJJM9sznrmwhAuj511oe4t1jAqAQ3D6Wc8Ic`),
    the rule that a token is needed only for creating, modifying and private data,
    a scope per request, 300 requests per minute per IP whether or not a token is
    sent, and a mandatory unique `User-Agent`.
  * `https://docs.modrinth.com/guide/oauth/` -- the authorizer is
    `https://modrinth.com/auth/authorize`, the exchange is `POST
    https://api.modrinth.com/_internal/oauth/token` with `application/x-www-form-urlencoded`
    and the client secret in the `Authorization` header, the response is
    `{access_token, token_type: "Bearer", expires_in}`, scope identifiers live in
    `apps/labrinth/src/models/v3/pats.rs`, and it opens by saying *"If the only user
    of the application is yourself, a personal access token (PAT) may be a better
    fit."*

  **What this corrects.** G105 recorded the profile page, the Servers page and the
  panel's two sections as "behind a Modrinth sign-in this launcher does not have",
  which is right, and then called the alternative "a slice of its own size" without
  measuring the slice. Two things about that framing do not survive the reading.
  First, `_internal` is not by itself a mark of unreachability: Modrinth's published
  OAuth guide *documents* `api.modrinth.com/_internal/oauth/token` as the exchange a
  third-party application is supposed to call. What is app-internal is the
  **namespace** (`billing_internal`, `campaign_internal`, `attribution_internal`,
  `moderation_internal`, `external_projects_internal`) and whether a user token is
  scoped for it -- which the documentation says is a per-request scope question, so
  a wrong scope answers 401 rather than 404. Second, "a slice" is the wrong unit for
  the Servers half: that page's data is not Labrinth at all. It is **Archon**, a
  second host (`https://archon.modrinth.com`) with eleven versioned namespaces and
  67 distinct methods, a websocket (`archon.sockets.on` five times in one composable)
  and an SFTP handoff (`servers_v0.getFilesystemAuth`) -- and `billing_internal.getProducts`
  is only the price list beside it.

  **What a session is, in the reference's own words.** `plugin:mr-auth` answers
  `{session, expires, user_id, active}` and the frontend sends it as
  `Authorization: Bearer <session>` (`App.vue:1591`) against `labrinthBaseUrl`, and
  as the bare token in `App.vue:1336`'s session check against `/v2/user`. It is not
  the Microsoft account and shares nothing with it: that one's token signs a launch,
  this one authenticates Labrinth and Archon. The plugin's own Rust -- the OAuth
  client id, the redirect URI, the scopes it asks for, the refresh -- is what is
  **not** in this tree, so how the reference *obtains* the session is not
  measurable here; how it *uses* one is.

  **What each remaining surface would call.** Servers and an instance's hosting
  half: the eleven Archon namespaces above plus `billing_internal.getProducts` for
  the catalogue, i.e. not two slices' worth of work but a client of its own. The
  panel's fundraiser banner: `campaign_internal.getPride26()`, one call. The friends
  list's signed-in half: four `plugin:friends` commands in the reference, whose user
  documents come from the *published* `users_v2.getMultiple` -- and Labrinth's own
  `friends_v3.list()`/`add()` are what the hosting page's access tab uses, so a
  session could reach the same data two ways. The profile page's own half: the
  reader's own identity is `users_v3.getAuthenticated()`, which the app reads at
  startup (`App.vue:349`) and again for the Skins page's Modrinth store
  (`Skins.vue:272`) -- so the sentence this launcher's profile page cannot select
  (`State::empty_sentence(true)`, G108) is one authenticated call away.

  **Where a token would have to live.** Not in `accounts.json`: that file is
  Prism's, it is read and written field by field so as not to sign a user out of the
  other launcher (`crate::accounts` says why in its module docs), and Modrinth has
  no meaning in Prism's schema. The launcher's own file is
  `PalantirPaths::home`/`crate::prefs`, which is already written atomically and is
  explicitly *not* the data root -- and a token there is a plaintext secret, which is
  the cost that has to be named rather than discovered. The reference keeps its own
  credentials in the plugin's store, which is another thing this tree cannot see.

  **The decision this leaves, stated as three options rather than a recommendation.**
  (A) An OAuth2 application of this launcher's own: register an application with
  Modrinth (name, scopes, allowlisted redirect URIs -- a human step, and a client
  secret this project would then have to hold), then a loopback listener, the
  urlencoded exchange, a token store and a re-authorize path when `expires_in` runs
  out; the published exchange has no refresh token, so "expired" means "authorize
  again". (B) A personal access token the reader generates in their own settings and
  pastes: no registration, no redirect, the documented `Authorization: mrp_…` header,
  at the cost of a settings field and a secret in a file, and with the open question
  this measurement cannot close -- whether Archon accepts the same token, since
  Archon's own documentation is not in this tree and neither is the client code that
  authenticates to it. (C) Leave all three surfaces recorded as out of reach, which
  is what the ledger's two open stage-3 items already say.

  **What this gate does not do.** It builds nothing and decides nothing: no source
  file changes, an external reading of two published pages, and the plan's two
  stage-3 items left open with their reasons intact. Its value is that whichever of
  the three options a reader takes, the size of what follows is now a number -- 16
  Labrinth namespaces, 11 Archon ones, 67 Archon methods, two plugin call sets --
  rather than the phrase "a slice of its own size".

  **The runner could not be the receipt.** This slice is documents and one reading
  of two published pages, so its push is refused the same way every other one is:
  run `36452778337`, `Lint` failed in 4 s and `Test workspace` in 3 s with **zero
  steps** and the same billing annotation, `Live services` and `Build exe` skipped.
  Nothing here was measured on the runner, and nothing here needed to be -- there is
  no code in it, and the only checks this slice owes are the two document tools,
  which exit 0 at 111 gates.

- [x] G110: the profile page's two documents are checked against the live service, and
  the three things that turned up while checking them are written down
  CHECK: curl -sS -H "User-Agent: PalantirMC/0.1.0-g110-measurement" \
              -o .scratch/g110-<name>.json -w '%{http_code}' \
              https://api.modrinth.com/v2/user/{modrinth,jellysquid3,2REoufqX/projects,jellysquid3/projects,}
         python -c "json.load(open(...))"                      # the fields the page reads
         curl -sS -H 'X-Panel-Version: 1' https://archon.modrinth.com/
         curl -sS -X POST -d 'grant_type=authorization_code&code=x&client_id=x' \
              https://api.modrinth.com/_internal/oauth/token
         python tools/progress.py --check
         python tools/dashboard.py --check
  EXPECT: 200 for the four documents, 401 for the reader's-own route without a token
          `name` null on **both** accounts, not only the official one
          the projects list keyed `id`, with every field the row draws
          a client id, not a token, is what OAuth's exchange asks for
          and Archon: 426 for every path, 200 once `X-Panel-Version: 1` is sent
  EVIDENCE: the transcript of these requests from this machine, 2026-09-28 (the
            bodies are in `.scratch/g110-*.json`, which is ignored):

```
$ curl -H "User-Agent: PalantirMC/0.1.0-g110-measurement" -w '%{http_code}' ...
200  https://api.modrinth.com/v2/user/modrinth
200  https://api.modrinth.com/v2/user/jellysquid3
200  https://api.modrinth.com/v2/user/2REoufqX/projects
200  https://api.modrinth.com/v2/user/jellysquid3/projects
401  https://api.modrinth.com/v2/user

$ python -c '...'          # the fields G108 reads, out of the live documents
== user/modrinth   keys: auth_providers, avatar_url, badges, bio, created, email,
                        email_verified, github_id, has_password, has_totp, id,
                        name, payout_data, role, username
   id 2REoufqX  username Modrinth  name None  role admin  badges 1
   avatar_url https://cdn.modrinth.com/data/2REoufqX/...96.webp
   bio "An official user account of Modrinth. support@modrinth.com"
   created 2023-11-13T23:22:36.604990Z
   email None  email_verified None  payout_data None  github_id None
   has_password None  has_totp None  auth_providers None
== user/jellysquid3  id TEZXhE2U  username jellysquid3  name None  role developer
   badges 0  bio "Professional idiot at day, maniac programmer by night."
   created 2021-01-03T00:49:18.373336Z
== user/jellysquid3/projects  4 entries, all project_type "mod", keyed id
   Sodium, Hydrogen, Lithium, Phosphor
   first: id AANobbMI  slug sodium  title Sodium  downloads 232693403
          icon_url .../AANobbMI/...96.webp  published 2021-01-03T00:53:34.185936Z
   38 keys, the whole project document; no null icon_url in any of the 4
   sum(downloads) 364662512
== user/2REoufqX/projects  []
== /v2/user (no token)  401
   {"error":"auth_error","description":"flattening v2 not-found response",
    "details":["authenticating API request","Authentication method was not valid"]}

$ curl -H 'X-Panel-Version: 1' https://archon.modrinth.com/
200
modrinth/archon 0.1.1 (eecc398) [build]
compiled 23 minutes ago

$ curl https://archon.modrinth.com/v0/servers          # no version header
426  {"error":"unsupported archon request version"}
$ curl -H 'X-Panel-Version: 1' https://archon.modrinth.com/v0/servers
404  not found

$ curl -X POST -d 'grant_type=authorization_code&code=x&client_id=x' \
       https://api.modrinth.com/_internal/oauth/token
400  {"error":"invalid_client","description":"The provided client id was invalid"}
```

  **What this closes.** G108's own limit, stated in its gate: *"The live tests are
  the ones that would prove the two new documents against the service."* They still
  are -- `palantir-net/tests/live.rs` has no profile test yet -- but the documents
  themselves are now measured by hand against the live API, field by field, and
  they are what the code parses: the same `id`, `username`, `name`, `avatar_url`,
  `bio`, `created`, and the projects list keyed `id` with `project_type`, `title`,
  `description`, `downloads`, `icon_url` and `published`. Two details are stronger
  than the fixtures they were written from: **`name` is null on an ordinary account
  too** (`jellysquid3`, role `developer`), so the nullable field is not a quirk of
  the official one; and the sum the header draws is real arithmetic over a real
  list -- 364,662,512 downloads across Sodium, Lithium, Phosphor and Caffeine.

  **Finding one: the private fields are present and null.** The published user
  document carries `email`, `email_verified`, `payout_data`, `github_id`,
  `has_password`, `has_totp` and `auth_providers` -- and every one of them is `null`
  for a read with no token. That is a third reason `null_as_empty` exists rather
  than a `#[serde(default)]` alone, and it is also the answer to a question the page
  never asks: a profile's `role` *is* public (`admin`, `developer`), so "is this
  profile the reader's own?" cannot be answered from it. Only a token answers that.

  **Finding two: Labrinth's errors do not look like Minecraft's.** This service
  answers `{"error", "description", "details"}` where the skin service G106 writes
  answers `{"errorMessage": ...}`. The engine surfaces `HTTP <status>` for both, so
  a refused Modrinth request currently loses Labrinth's own sentence -- "The
  provided client id was invalid" above -- and keeps only the number. Nothing here
  changes that; it is a one-line improvement in the error path, named so it is not
  rediscovered.

  **Finding three, and the correction to G109.** G109 asked whether Archon accepts a
  personal access token and could not answer it from the tree or the published
  pages. The first thing Archon wants is not a token at all: **every** path answers
  `426 unsupported archon request version` -- with or without a `Bearer` header --
  until the request carries `X-Panel-Version: 1`, and then the root answers
  `modrinth/archon 0.1.1 (eecc398) [build]`, a sibling of Labrinth identifying itself
  the same way. The header comes from the api-client's `panel-version.ts`, which is
  the point: **the package that would answer the rest is not vendored here.**
  `app-frontend`'s `package.json` names `"@modrinth/api-client": "workspace:^"`, and
  `UPSTREAM.md` records the copy as `app-frontend`, `ui`, `assets` and
  `tooling-config` -- so the client's `src/modules/{archon,iso3166,kyros,labrinth,launcher-meta,mclogs,paper,purpur,shared-instances}`
  is one directory upstream (`modrinth/code`, the same commit `8966b5e`) and not in
  this tree. It is where the base URLs, the auth feature
  (`new AuthFeature({ token: ... })`), the version segment
  (`/_internal`, `/v{n}`, `'/v0'`) and every Archon path are written down. Reading it
  from upstream to write this gate is what makes the Archon half of the Servers
  item measurable at all; **vendoring it the way the frontend was vendored** is a
  decision for whoever takes that item next, and it is a smaller decision than the
  sign-in one.

  **And the exchange route is live.** `POST api.modrinth.com/_internal/oauth/token`
  with a form body and no client secret answers `400 invalid_client` -- not 404 --
  so the guide's exchange is deployed and the thing it gates on is a registered
  application's credentials. That is as far as a measurement without an account can
  go, and it is one step further than G109 left it (G109: the route is documented;
  G110: the route answers).

  **What this does not settle.** Whether a personal access token is accepted by
  Archon, and by the `_internal` namespaces, is still unmeasured: a token is
  somebody's account, and nothing in this ledger has ever used one. Every request
  above is unauthenticated, from a launcher that stores no Modrinth credentials.
  The profile page's reader's-own half stays out of reach for the same reason, and
  the number of requests this machine made to measure all of it is eleven.

  **The push.** No build was run for this gate, because the machine was busy at the
  user's request: everything above is tree reads, live reads and the two document
  tools (`progress.py --check` / `dashboard.py --check` exit 0 at 112 gates).
  `1bc507e` pushed; run `36453808658` is the same billing block as G101-G109 --
  `Test workspace` 3 s and `Lint` 2 s, zero steps, `Live services` and
  `Build exe` skipped, annotation *"The job was not started because recent account
  payments have failed or your spending limit needs to be increased"*. Which means
  G108's caveat still stands and is the one thing owed: the workspace suite has not
  been re-run since the G120 locales slice was rebased under it, so re-running
  `cargo test --workspace --all-targets --locked` and clippy on this tree is the
  first step of the next compiled slice, before its own numbers are trusted.
- [x] G121: the language setting -- the module that reads a table, the fallback, the
  direction, the Settings row, and a renderer that never touches English
  CHECK: python tools/gen_text.py --check
         python tools/gen_locale.py --check
         cargo test --workspace --all-targets --locked
         cargo clippy --workspace --all-targets --locked -- -D clippy::correctness
  EXPECT: both generators report byte-identical output
          1004 passed; 0 failed; 17 ignored, between the seven suites
          clippy exit 0 with 42 warnings -- G120's own count, unchanged, and none
          of them in `locale.rs`, in `text.rs` or about `button_text`
  EVIDENCE: the transcripts of these commands on this tree:

```
$ python tools/gen_text.py --check
text generation is byte-identical

$ python tools/gen_locale.py --check
locale generation is byte-identical

$ cargo test --workspace --all-targets --locked
    177 passed; 0 failed  (palantir-core, lib)
      8 passed; 0 failed  (palantir-core, tests/compat.rs)
    537 passed; 0 failed  (palantir-desktop, bin)
      4 passed; 0 failed  (palantir-desktop/tests/native.rs)
     31 passed; 0 failed  (palantir-loader, lib)
    247 passed; 0 failed  (palantir-net, lib)
      0 passed; 0 failed; 17 ignored  (palantir-net, tests/live.rs)

$ cargo test -p palantir-desktop --locked --bin PalantirMC -- locale:: text::
test locale::tests::a_chosen_tag_survives_and_names_its_own_table ... ok
test locale::tests::a_fractional_category_is_not_produced_for_an_integer_count ... ok
test locale::tests::english_is_the_default_and_an_unknown_tag_opens_as_it ... ok
test locale::tests::a_translated_key_is_the_locale_s_own_sentence ... ok
test locale::tests::a_key_a_locale_does_not_carry_falls_back_and_a_key_it_does_does_not ... ok
test locale::tests::every_category_the_rule_produces_is_one_the_generator_compiles_against ... ok
test locale::tests::the_language_in_force_is_the_one_the_rule_reads ... ok
test locale::tests::a_language_is_labelled_with_the_reference_s_own_name ... ok
test locale::tests::the_offer_is_the_reference_s_own_list_and_ar_sa_is_not_in_it ... ok
test locale::tests::the_direction_is_the_reference_s_own_field ... ok
test locale::tests::the_plural_rule_is_each_language_s_own ... ok
test text::tests::a_key_a_locale_does_not_carry_renders_english ... ok
test locale::tests::numbers_are_grouped_the_way_each_language_groups_them ... ok
test text::tests::a_number_is_grouped_by_threes_from_the_right ... ok
test text::tests::a_category_is_its_own_answer ... ok
test text::tests::an_exact_arm_matches_the_number_and_nothing_else ... ok
test text::tests::english_never_goes_through_the_locale_renderer ... ok
test text::tests::a_tagged_message_splits_around_its_slot ... ok
test text::tests::the_english_rule_is_one_for_one_and_other_for_everything_else ... ok
test text::tests::the_two_renderings_differ_for_a_number_and_agree_for_a_category ... ok
test text::tests::a_category_the_caller_chose_is_not_pluralized_again ... ok
test text::tests::a_locale_s_own_plural_arms_are_the_ones_that_render ... ok
test text::tests::every_offered_language_renders_every_shape_without_a_brace_left_in_it ... ok
test result: ok. 23 passed; 0 failed; 0 ignored; 0 measured; 514 filtered out; finished in 5.23s

$ cargo clippy --workspace --all-targets --locked -- -D clippy::correctness
$ echo $?
0
$ grep -c '^warning: ' .scratch/g121-clippy.log
42
$ grep -c 'locale.rs\|text.rs\|button_text' .scratch/g121-clippy.log
0
```

  The eighteen tests this slice adds are twelve in `locale`, five in `text` and one
  in `shell` -- the Settings row's own, which the filter above does not name. On this
  slice's own base the suite went from G120's 970 to 988 and the desktop binary from
  507 to 525. The transcript above is the **merged** tree instead, because the rebase
  put the profile page, the session measurement and the live check under this slice:
  their 16 tests -- twelve in the desktop binary and four in `palantir-net` -- are
  the difference, 988 + 16 = 1004, and 525 + 12 = 537.

  **The claim this slice is built on.** English is **not** rendered through the new
  renderer. `text::render` returns `None` whenever the language in force is English
  or the locale does not carry the key, and every generated helper runs the code
  the generator wrote when it does. So the 3,846 sentences this launcher draws today
  are the same code paths they were before this slice, and a second language is an
  addition rather than a rewrite of what an English reader sees. A test holds the
  hardest version of that: with English in force, `render` is `None` even when given
  arguments.

  **What is in the module.** `locale.rs` is the tag, the sparse-table lookup, the
  fallback, the direction of the table in force, the reference's own 32-code offer,
  and the CLDR plural rule -- the runtime half of the table `tools/gen_locale.py`
  compiles against. The choice is ambient and **per thread**: iced runs
  `update`/`view` on the thread that started the `Application`, so one window has
  one value, and the reason for the thread-local is the test suite rather than the
  window -- a process-wide value would let one test's `set("de-DE")` put German in
  force for a render test running beside it, and a suite that fails depending on
  how a runner scheduled two threads is worse than no test. `prefs.rs` already had
  `locale: Option<String>` -- it was declared before there was a table to name -- so
  this slice reads and writes a field that existed; English is stored as the empty
  string because the preferences file is the diff from the defaults.

  **The renderer, and the two things it does not do.** `text::render` walks a
  locale's template in one recursive pass -- no AST, because an arm's body is a
  slice of the same string -- and fills in `{name}`, `{name, number}`, plural arms
  with `#`, and selects, with each argument carried by *kind* rather than as a
  string (a locale may write `{count, number}` where English writes `{count}`). Two
  limits are real and are recorded rather than approximated:

  1. **Number grouping is each language's own; digit shapes are not.** A `#` or a
     `{count, number}` groups with the language's separator -- `1.234` in German
     and Portuguese, a no-break space in Russian, Polish and French, a comma in
     English and the CJK languages -- but every digit is a Western one. The
     reference's `Intl.NumberFormat('ar-SA')` writes 1,234 as `١٬٢٣٤`, and this
     renderer writes `1,234`, so an Arabic sentence here carries Arabic words with
     Western numerals. The reason is proportion, not impossibility: 32 messages
     carry a typed number and the difference only shows at four digits and above,
     and a digit-shape table is one nobody in this slice measured.
  2. **French's separator is the ordinary no-break space.** Modern CLDR uses a
     *narrow* one for French, so a grouped French number here may differ from a
     browser's by that one code point.

  **One correction, and one refinement of G120.** The correction: this slice first
  recorded that the English locale names only 31 of the 32 offered codes and that
  `es-419` was the one it did not. That was wrong, and it was a grep pattern rather
  than the data -- `"locale\.[a-zA-Z-]+"` does not match a digit, so `es-419` was
  invisible to the search and visible to the compiler. All 32 `locale.*` names
  exist, `label` resolves every offered code, and the `Option` in the generated
  table is a guard for a future code rather than a case today. The refinement:
  `gen_locale.py --report` lists the arms a language's *category set* does not
  contain, which cannot see that Czech `many` and Polish `other` belong to those
  languages' **fractional** rules (`v != 0`). Every count this launcher passes to a
  rule is an integer, so those two arms are unreachable here -- and unreachable in
  the reference for the same reason, since `Intl.PluralRules` is handed the same
  integers. The report's own label was reworded to say what it computes, and the
  integer-level version is a test in `locale`.

  **What is deliberately not built.** The reference's language *page* has a search
  field, a category list and a site/app platform switch; this launcher's Settings is
  a modal, so what it offers is the list of languages and the reference's own
  warning sentence, and not that chrome. And nothing lays out from the direction
  flag yet: `Direction` and `is_rtl` are `#[allow(dead_code)]` with the reason at
  the definition, because G128 is what mirrors the shell on them.

  **One thing two rebases did to a document.** This slice's diff to `NEXT_STEPS.md`
  is 977 lines and one paragraph of it is new content. That file is stored with LF
  here -- LF at `a4b5725`, and this clone checks out CRLF because `core.autocrlf` is
  on -- but the commits landed under this slice from `1bc507e` to `568be5d` stored it
  with CRLF, one of them with a stray `\r` inside a sentence (`is\r\r\n  still
  unclaimed`). A conflict on every line of a document is an ending conflict rather
  than a content one, so each was resolved as a three-way merge against the real base
  with the three copies normalised to LF first; the content merged cleanly both
  times. What the diff shows is that paragraph and the re-ending to LF, which is the
  convention `.gitattributes` states for this tree.

  The runner could not be the receipt either: this slice's push, `f152d94`, is run
  `36459102045`, the 81st consecutive failure on this branch since `36038333030` and
  the same block as the 80 before it -- `Test workspace` and `Lint` each failed with
  **zero steps** in 3 s (`17:34:39Z` to `17:34:42Z`) on
  `The job was not started because recent account payments have failed or your
  spending limit needs to be increased`, with `Live services` and `Build exe`
  skipped rather than scheduled. So this slice's receipt is the local run transcribed
  above: the same compiler and the same flags, but not the clean checkout `AGENTS.md`
  calls the authority, and nothing here should be read as "CI passed".

- [x] G111: what the api-client says Archon and a node are, read from upstream, the
  request-call count the registry holds, and the correction it makes to G110
  CHECK: curl -sS -A PalantirMC/0.1.0-g111-measurement \
              "https://api.github.com/repos/modrinth/code/contents/packages/api-client/src[/modules[/archon]]?ref=8966b5e"
         curl -sS "https://api.github.com/repos/modrinth/code/git/trees/74f004c7ae39d37f82fc79c0a930d91d626acc03?recursive=1"
         curl -sS -O "https://raw.githubusercontent.com/modrinth/code/8966b5e/packages/api-client/src/{types/client.ts,features/auth.ts,features/panel-version.ts,features/node-auth.ts,core/abstract-module.ts,utils/jwt-retry.ts,utils/node-url.ts}"
         curl -sS -O "…/src/modules/archon/{actions,backups,backups-queue,content,options,properties,server-users}/{v1}.ts" \
                    "…/src/modules/archon/{nodes,servers,transfers}/{internal}.ts" "…/src/modules/archon/{notices,servers}/v0.ts" "…/src/modules/archon/servers/v1.ts"
         python -c "re.findall(r'client\\.request', open(f).read())"      # per module, and the total
         curl -A PalantirMC/0.1.0-g111-measurement -o /dev/null -w '%{http_code}' \
              api.modrinth.com/v2/user/modrinth [-H 'X-Panel-Version: 1|99']
         curl -H 'X-Panel-Version: {1,99}' https://archon.modrinth.com/{,*v1*}{v1/servers,v1/regions}
  EXPECT: three base URLs, the sentinel header a constant sent to two of them, one token
          13 Archon modules and 84 request calls behind them, and 3 version segments
          Labrinth answers 200 whatever the header says; Archon refuses the wrong value
          a node's own auth, and the one Archon route that needs no token at all
  EVIDENCE: the upstream files (they are in `.scratch/g111-*.ts`, which is ignored)
            and the transcript of the live probes, 2026-09-28:

```
# packages/api-client/src — the package `app-frontend/package.json` names as
# "@modrinth/api-client": "workspace:^" and this tree does not vendor
https://api.github.com/repos/modrinth/code/contents/packages/api-client/src?ref=8966b5e2e7951e83651fbebbf2fb6d7608a94a33
  core/  features/  modules/  platform/  state/  tests/  types/  utils/  index.ts
  core/: abstract-client.ts abstract-feature.ts abstract-module.ts abstract-sync.ts
         abstract-upload-client.ts abstract-websocket.ts errors.ts
  features/: auth.ts circuit-breaker.ts node-auth.ts panel-version.ts retry.ts verbose-logging.ts

$ python -c '...'    # the file trees under modules/, and modules/archon
modules/: archon/ iso3166/ kyros/ labrinth/ launcher-meta/ mclogs/ paper/ purpur/
          shared-instances/ index.ts types.ts
modules/archon/: actions/ backups/ backups-queue/ content/ nodes/ notices/ options/
                 properties/ server-users/ servers/ transfers/ index.ts types.ts
modules/kyros/:  content/ files/ logs/ upload-sessions/ types.ts

$ python -c '...'    # `client.request` calls per Archon module file, and the methods
  actions-v1          1 calls  GET                    35 lines
  backups-queue-v1   10 calls  DELETE,GET,POST       113
  backups-v1          7 calls  DELETE,GET,PATCH,POST 113
  content-v1         19 calls  GET,POST              290
  nodes-internal      1 calls  GET                    20
  notices-v0          6 calls  DELETE,GET,PATCH,POST,PUT 98
  options-v1          2 calls  GET,PATCH              37
  properties-v1       2 calls  GET,PATCH              40
  server-users-v1     5 calls  DELETE,GET,PATCH,POST  83
  servers-internal    1 calls  GET                    23
  transfers-internal  4 calls  GET,POST               84
  servers-v0         19 calls  DELETE,GET,POST,PUT   321
  servers-v1          7 calls  DELETE,GET,POST       102
  total 84 calls in 13 modules (58 in the 11 above + 19 + 7)

$ grep -rho "client\.archon\.[a-zA-Z0-9_]*\.[a-zA-Z0-9_]*" vendor/… | sort -u | wc -l
65          # two-name call sites in the vendored frontend; 67 by G109's deeper pattern

$ curl -o /dev/null -w '%{http_code}' api.modrinth.com/v2/user/modrinth
no header 200    X-Panel-Version: 1 200    X-Panel-Version: 99 200

$ curl -H 'X-Panel-Version: …' https://archon.modrinth.com/…
/v0/servers  no header   426 {"error":"unsupported archon request version"}
/v0/servers  version 1   404 not found
/v0/servers  version 99  426 {"error":"unsupported archon request version"}
/v1/servers  version 1   401 {"error":"unauthorized","description":"you are not authorized to view this resource"}
/v1/regions  version 1   200 [{"shortcode":"au-syd","countrycode":"au","display_name":"Sydney, Australia",
                             "lat":-33.903355933,"lon":151.19163831,"zone":"nodes.modrinth.com",
                             "bucket_regions":["australia-southeast"],"internal":false,
                             "backup_anchor_hour":0}, {"shortcode":"us-vin", …}]
```

  **The client's own contract, in its own words.** `types/client.ts` names three
  hosts, not two: Labrinth defaults to `https://api.modrinth.com`, Archon to
  `https://archon.modrinth.com`, and a third to
  `https://shared-instances.modrinth.com`, with a `timeout` of 10000 ms, an
  `archonSentryCapture` that attaches `modrinth-sentry-capture: 1`, and
  `features?: AbstractFeature[]` as the only way auth gets in. `features/auth.ts`
  is that one way: a single `token` -- static string or async provider -- written as
  `Authorization: Bearer <token>` (prefix and header name are configurable), skipped
  when the header is already set or the request sets `skipAuth`. **Nothing in it
  distinguishes a personal access token from an OAuth access token, and nothing
  distinguishes Labrinth from Archon** -- the same feature object stamps whatever the
  client sends. So G109's open question stays open on the server side, but it is no
  longer a question about the client: this launcher would hold one secret and send it
  to both hosts, which is exactly what a personal access token would need to be
  accepted for.

  **The version header is a client stamp, not an Archon rule -- correcting G110.**
  `features/panel-version.ts` is eighteen lines: `export const PANEL_VERSION = 1`, and
  `shouldApply` returns true when `context.options.api` is `'labrinth'` **or**
  `'archon'`. Measured against both services: Labrinth answers 200 to no header, to
  `1` and to `99`, so it ignores the value entirely; Archon answers 426 to no header
  and to `99`, 404 to `1` on `/v0/servers`, 401 to `1` on `/v1/servers`. G110 said the
  header is "what Archon wants before anything else", which is true of Archon but
  mistaken about why: it is the client telling the service *which contract this panel
  speaks*, the same header going to both, and only one of the two services acting on
  it. The number is a constant 1, so a Rust client sends `X-Panel-Version: 1` on every
  Archon request and nothing on Labrinth's unless it wants to.

  **Archon is thirteen modules, not eleven, and its surface is 84 request calls.**
  G109 counted eleven namespaces and 67 methods out of the *vendored frontend's* call
  sites (`client.archon.*`, which the two-name pattern counts as 65 and the deeper one
  as 67). The registry is the API's own answer: `modules/index.ts` maps thirteen
  `archon_*` keys -- eleven of them `_v1`/`_v0`/`_internal` by name -- to module
  classes, and `modules/archon/index.ts` re-exports only six groups (actions, backups,
  backups-queue, content, properties, servers), leaving nodes, notices, options,
  server-users and transfers reachable through the registry without being part of the
  package's public surface. Counting `client.request` in the thirteen module files
  gives **84 calls**, the largest being content (19) and the servers pair (19 + 7).
  Servers exist at **two versions at once** -- `servers/v0` (19 calls: `/servers`,
  `/servers/:id`, `/stock`, `/servers/:id/{fs,ws}` answering a JWT and a websocket
  auth, `power`, `reinstall`, `reinstallFromMrpack`, `name`, four `/allocations` verbs,
  `/subdomains/:s/isavailable`, `/subdomain`, `/startup`, `/notices/:id/dismiss`) and
  `servers/v1` (7: list, get, `select-download`, `regions`, `flows/intro` DELETE,
  `worlds/:wid/onboard` POST, `sftp/roll` POST) -- and the version segment is per
  request, not per module: `/v0`, `/v1` or `/_internal`.

  **A node is a fourth thing.** `features/node-auth.ts` and `modules/kyros/`
  (content, files, logs, upload-sessions) are the file half of a hosted server: a
  per-server JWT from `/v0/servers/:id/fs`, written into the same `Authorization`
  header, against a *node* host (`node-xyz.modrinth.com/modrinth/v0/fs`, whose base URL
  `utils/node-url.ts` recovers by stripping `/modrinth/v{n}/fs` and defaulting to
  https), with a 401 loop that refreshes and retries three times and a `wss://`
  rewrite for the websocket. `utils/jwt-retry.ts` is the one-shot version of the same
  idea: on 401 call `refreshToken()`, retry once.

  **And one Archon route answers a stranger.** `/v1/regions` is marked `skipAuth: true`
  in `servers/v1.ts` and it is true on the wire: no token, no account, 200 with real
  bodies -- Sydney and Vint Hill so far, each with `shortcode`, `countrycode`,
  `display_name`, `lat`/`lon`, `zone` (`nodes.modrinth.com`), GCP `bucket_regions`,
  an `internal` flag and `backup_anchor_hour`. It is the first byte of Archon this
  launcher can read today, and it is also the honest shape of everything else: 401
  `{"error":"unauthorized","description":"you are not authorized to view this
  resource"}` -- Labrinth's error shape again, so G110's finding two covers Archon too,
  and the engine's `HTTP <status>` would drop Archon's own sentence as well.

  **What this settles and what it does not.** The Servers page's own data stays behind
  a token: `/v1/servers` is per-account and answers 401 without one. What is settled is
  the contract a Rust client would have to speak, its size (84 calls across 13 modules,
  three version segments, a fourth node host with a JWT of its own), and that the
  first step is derivable without an account. What is not settled is whether a `mrp_`
  personal access token is accepted by Archon or by the `_internal` namespaces -- both
  401s above are "no token", not "wrong kind of token" -- and that still needs an
  account, which this launcher has none of.

  **The push.** `7925681`; run `36454610897` is the billing block again -- zero steps
  on `Test workspace` and `Lint`, `Live services` and `Build exe` skipped, the same
  annotation as every push since G101. No build was run for this gate either, since
  upstream reads and live probes compile nothing, so G108's debt still stands: the
  workspace suite and clippy have not been re-run since the locales slice was rebased
  under them.

- [x] G112: a refusal keeps the service's own sentence, and the metadata store's
  fetcher stops calling a 404 a transport failure (the two findings G110 and G111 named)
  CHECK: cargo test --workspace --all-targets --locked
         cargo clippy --workspace --all-targets --locked -- -D clippy::correctness
         cargo test -p palantir-net --test live --locked -- --ignored \
               a_real_refusal_carries_its_status_and_whatever_sentence_the_service_sent --nocapture
         curl -sS -w '[%{http_code}] %{size_download} bytes' \
              api.modrinth.com/v2/{user,tag/nonexistent-loader,project/sodium/version/999999999}
         curl -sS -H 'X-Panel-Version: 1' archon.modrinth.com/v0/servers
  EXPECT: the workspace suite green on the tree G108 left, then on this one, then
          on the merged tree: 986, 990 and 1008 passed, 0 failed in all three
          clippy exit 0 every time, at the same 42 warnings
          a 404 whose body carries JSON says so; one whose body is empty says only the code
          Archon's two words `not found` arrive as the sentence
  EVIDENCE: the runs on this tree, and the live test's own output, 2026-09-28:

```
# first, the debt G108's gate recorded: the merged tree, no slice on top
cargo test --workspace --all-targets --locked
  test result: ok. 177 passed; 0 failed; 0 ignored        (palantir-core)
  test result: ok.   8 passed; 0 failed; 0 ignored        (palantir-core, integration)
  test result: ok. 519 passed; 0 failed; 0 ignored        (palantir-desktop)
  test result: ok.   4 passed; 0 failed; 0 ignored        (palantir-desktop, integration)
  test result: ok.  31 passed; 0 failed; 0 ignored        (palantir-loader)
  test result: ok. 247 passed; 0 failed; 0 ignored        (palantir-net)
  test result: ok.   0 passed; 0 failed; 17 ignored       (palantir-net, live)
exit 0 -> 986 passed / 0 failed / 17 ignored
cargo clippy --workspace --all-targets --locked -- -D clippy::correctness
exit 0, 42 warning lines, the set identical to G106's

# then the same two commands with the slice on disk
cargo test --workspace --all-targets --locked
exit 0 -> 990 passed / 0 failed / 18 ignored
  (net 247 -> 251: the four new unit tests; live 17 -> 18: the one new live test)
cargo clippy --workspace --all-targets --locked -- -D clippy::correctness
exit 0, 42 warning lines, the set identical again
cargo clippy -p palantir-net --all-targets --locked -- -D clippy::correctness   # forced fresh
exit 0 in 1m 28s -- "Checking palantir-net", 13 warning lines, no correctness denial

# the live test, against both services, --nocapture
json:       http error for https://api.modrinth.com/v2/tag/palantirmc-live-no-such-loader:
            http status 404: the requested route does not exist
empty body: http error for https://api.modrinth.com/v2/project/sodium/version/999999999:
            http status 404
archon:     http error for https://archon.modrinth.com/v0/servers:
            http status 404: not found
test result: ok. 1 passed; 0 failed; 0 ignored; 17 filtered out; finished in 1.64s

# and the three bodies by hand, because the URLs were chosen from measurements
/v2/user                                    401  150 bytes  {"error":"auth_error","description":
     "flattening v2 not-found response","details":["authenticating API request","Authentication
     method was not valid"]}
/v2/tag/nonexistent-loader                  404   72 bytes  {"error":"not_found","description":
     "the requested route does not exist"}
/v2/project/sodium/version/999999999        404    0 bytes  (nothing at all)
archon /v0/servers, X-Panel-Version: 1      404    9 bytes  not found
```

  **What changed, and why it is a slice rather than a tidy-up.** G110 and G111 each
  ended by naming the same defect: the engine renders every refusal as
  `http status <code>`, so Labrinth's "The provided client id was invalid" and
  Archon's "you are not authorized to view this resource" both reach the reader as
  `401`. A page shows that string -- `Store` maps every engine error with
  `error.to_string()` -- so this is the reader-visible half of both findings.

  `Error::status_with(url, status, sentence)` is the new constructor and
  `Error::status` is now it with no sentence, which is why the old string is
  byte-for-byte unchanged when a service sent nothing: every existing test and log
  line still holds. The pool reads up to 4 KiB of the refusal's body in both places
  a non-2xx becomes an error (`send`, which a whole-body read and a download both
  go through, and the conditional read) and `failure_sentence` looks for
  `description`, then `errorMessage`, then `error` -- the two measured shapes --
  falling back to a short, readable, non-markup text body. `details` is deliberately
  not read: it is Labrinth's internal chain, and on the one measured 401 that carries
  it the summary above the chain says less than the chain does. A sentence is bounded
  at 200 characters with an ellipsis, and markup, binary and over-long bodies are
  refused rather than pasted into a notice.

  **Two things the work turned up that the measurements had not.** First, **an empty
  404 is real**: Labrinth answers a *matched* route with a missing resource
  (`/project/sodium/version/999999999`) with `Content-Length: 0`, so "no sentence" is
  an ordinary case and not a fallback for a broken body. The live test asserts exactly
  that string, unchanged. Second, `BlockingHttpFetcher::get` -- the metadata store's
  own fetcher, a second place in this crate where a non-2xx became an `Error::Http` --
  was building it by hand as `Error::http(url, format!("http status {status}"))`,
  which leaves `status: None`. A metadata 404 therefore arrived as a failure the retry
  line could not tell from a dropped connection (`is_retryable` reads the field, and
  `None` is retryable). It now uses the same two helpers and the same constructor, so
  there is one answer in this crate to "what did the service say".

  **The first version of the live test failed, and that is in here on purpose.** It
  asked for a user name nobody holds and asserted a sentence arrived; the service
  answered `404` with a zero-byte body, so the assertion was wrong about the service
  rather than the code about the string. Measuring that URL (`curl` above) is what
  turned the test into the three-shape one -- and into the finding that "no sentence"
  has to be a *tested* outcome rather than an overlooked one.

  **The merged tree's own numbers, because the two runs above predate a rebase.**
  The three locale slices landed on `rewrite-modrinth-native` while this slice was
  being written, so the push had to be rebased onto them; the conflicts were in
  `NEXT_STEPS.md` and `tools/progress.py` only (the locale work had reflowed the
  plan document from end to end, so all of it conflicted and the resolution was
  their version plus this slice's two paragraphs), and no source file conflicted.
  The suite and clippy were then re-run on the tree as it stands -- `b8a76b2`:
  `cargo test --workspace --all-targets --locked` exit 0, **1008 passed / 0 failed /
  18 ignored** (the desktop's own 519 grew to 537 with the locale tests, and net is
  251) and `cargo clippy --workspace --all-targets --locked -- -D clippy::correctness`
  exit 0 at 42 warnings whose texts are identical to G106's. That is the number this
  gate's push is behind; the 986 and 990 above are the two states before the merge.

  **What this does not do.** It does not change any message this launcher writes for
  itself, and it does not add Archon as a service: the live test spells out
  `archon.modrinth.com` rather than reading a constant, because this crate has no
  Archon base URL yet (G111 measured it as `https://archon.modrinth.com`), and the
  header the test sends is written in the test. Nothing here covers Minecraft's
  retry line, where `auth.rs` already reads `errorMessage` by hand in three places
  and could now share this helper -- named as the obvious follow-through rather than
  done, because a token-renewal path is the wrong place to refactor without a reason.

  **The push, and a new shape of it.** `b8a76b2`, and this time there is no run to
  point at: `gh api repos/MSedgeMC/PalantirMC/actions/runs?head_sha=b8a76b2...`
  answers `total_count 0`, where every push since G101 had at least produced a
  zero-step run to record. The newest run the PR holds is `36464361134` (`35ccb46`,
  the recording commit, created 18:19:18Z, `failure` in 5 s), and it is the same
  sentence again -- `The job was not started because recent account payments have
  failed or your spending limit needs to be increased` on both `Test workspace` and
  `Lint`, with `Build exe (${{ matrix.target }})` and `Live services` skipped at 0 s.
  A push that is not scheduled at all is one step past a run that starts nothing, and
  it is why this gate's numbers are the local suite's alone: no runner has seen this
  tree, and none will until the account can schedule jobs again. That is the same
  block G105's decision note already names, and it is what the mirror in that note is
  for.

- [x] G113: the live suite is green against the service again, with the stale
  premise that made it red written out of it
  CHECK: cargo test -p palantir-net --test live --locked -- --ignored --test-threads=1
         cargo test --workspace --all-targets --locked
         cargo clippy --workspace --all-targets --locked -- -D clippy::correctness
  EXPECT: all 18 live tests pass, none ignored, in about five minutes against the
          real services
          the workspace suite green at 1008 passed / 0 failed / 18 ignored, clippy
          exit 0 at the same 42 warnings
  EVIDENCE: the runner that found it -- `36575592704`, job `Live services`,
            2026-09-29 -- and the local runs after the fix:

```
# the first dispatch run on the public mirror, before anything was changed
---- a_created_instance_resolves_against_the_live_service stdout ----
thread 'a_created_instance_resolves_against_the_live_service' (4244) panicked at
  crates\palantir-net\tests\live.rs:119:5:
the instance no longer carries the versionless LWJGL slot this test is about
test result: FAILED. 17 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 82.86s

# the same test here, on the same tree -- so not a service that was slow or
# unreachable: it is over in 0.08 s, before a single request is made
test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 17 filtered out; finished in 0.08s

# after the fix
cargo test -p palantir-net --test live --locked -- --ignored --test-threads=1
test result: ok. 18 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 285.15s
exit 0
cargo test --workspace --all-targets --locked
exit 0 -> 1008 passed / 0 failed / 18 ignored
cargo clippy --workspace --all-targets --locked -- -D clippy::correctness
exit 0, 42 warning lines, the same set as G106's
```

  **What was red, and why nobody could see it.** The first `workflow_dispatch` on
  the public mirror is the first run since G101 in which a job actually executed,
  and it reported `Live services` red: 17 of the 18 live tests passed and
  `a_created_instance_resolves_against_the_live_service` panicked on its first
  assertion. Every other live test was green, which is what makes the second
  measurement the important one -- the same test here fails in **0.08 seconds**,
  before a request is made -- so the service was not the variable. The premise was.

  `Instance::create` reaches `PackProfile::vanilla`, and that function stopped
  writing a second component: piston keeps the LWJGL entries inside
  `net.minecraft` itself, so a reader of piston has nothing to put in an
  `org.lwjgl3` slot, and the comment where the code does it says exactly that.
  This test was written before the change and asserts the shape the launcher no
  longer writes. It went red at that change and stayed red, because `live` is
  gated on `github.event_name != 'pull_request'`: the job runs on a tag, on a
  `master` push or on a dispatch, and none of those had started since G101. **A job
  that never runs does not go red, it goes unread** -- which is the gap this gate
  closes rather than a fault in the test's design.

  **What it asserts now.** Three things, in the order they matter. That `create`
  writes exactly one component, `net.minecraft@<version>`, so a change to that
  shape fails here rather than quietly in the field. That resolution against the
  live service carries the instance the rest of the way regardless: main class,
  an asset index with a 40-character digest, LWJGL libraries *and* the natives
  this host can run, Java 21 among the compatible majors. And that the same
  instance carrying Prism's versionless `org.lwjgl3` slot -- the shape every
  instance already on disk has -- resolves that slot to a `3.`-something and puts
  **the same 32 `org.lwjgl` jars** on the classpath, so naming the slot cannot
  double it.

  **Two things the first attempt got wrong, recorded because they cost an hour.**
  The store is `meta.prismlauncher.org/v1` (`DEFAULT_META_BASE_URL`), not piston:
  its `net.minecraft` file carries *no* LWJGL entries and requires `org.lwjgl3`,
  so the slot is filled from that requirement (measured: `3.3.3`) and never
  reaches the `carried_elsewhere` path at all. That path is piston's, it has unit
  coverage in `resolve.rs`, and no live test with this store can reach it; the
  first version of this fix asserted it and the run answered in its own words --
  `disabled=false version="3.3.3"`. The second is that the job this was found in
  is in the same run as the release build: `package` built **both** Windows
  targets green there, `x86_64-pc-windows-msvc` and `x86_64-pc-windows-gnu`, so
  the mirror is not only a test runner, it is the delivery path G4 names.

  **What this does not do.** It does not put the live suite in front of a pull
  request, which costs about five minutes of runner per push and is not yet worth
  it; it does not add a piston-shaped live test, because a live test cannot choose
  its store's shape and the unit test already covers that half; and it does not
  explain why the mirror's creation push produced no run object where G112's push
  produced none either -- both are recorded as unread rather than as understood.

- [x] G114: a live assertion stops pinning a third party's revision of a
  document the third party republishes
  CHECK: cargo test -p palantir-net --test live --locked -- --ignored --test-threads=1
         curl -sS https://piston-meta.mojang.com/mc/game/version_manifest_v2.json
         # then the two revisions of asset index 17 that the failure named
  EXPECT: 18 live tests pass, none ignored, with both services naming the same
          index id and each naming its own digest in the URL it serves
  EVIDENCE: the runner that found it -- `36578642829`, job `Live services`,
            2026-09-29 -- and the two revisions, measured here:

```
# the runner, minutes after the same suite was green here
the_translation_agrees_with_the_mirror_the_shell_read: assertion `left == right` failed
  left: "https://piston-meta.mojang.com/v1/packages/9b16298b1dc0697878cec88bb2d96168f5239e4f/17.json"
 right: "https://piston-meta.mojang.com/v1/packages/de573f83da62843433ec9951c66feec7ed0a60a1/17.json"
test result: FAILED. 17 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 127.68s

# both of those revisions answer, and both are the same size
$ GET /v1/packages/de573f83.../17.json   -> 200  449557 bytes
$ GET /v1/packages/9b16298b.../17.json   -> 200  449557 bytes
  objects: 3911 in both; 0 keys only in one; 142 objects differ
    minecraft/lang/ig_ng.json  hash 7f80ca07... size 585152 -> 1dea0eae... 586816
    minecraft/lang/en_ud.json  hash b7b1291c... size 699325 -> 01b420d5... 699586
    minecraft/lang/ta_in.json  hash aee0e985... size 847089 -> a3e6e61b... 847331

# which side each service named, in the same hour
piston 1.21.1 version file, from here:  assetIndex.sha1 de573f83...
meta.prismlauncher.org/v1/net.minecraft/1.21.1.json: assetIndex.sha1 de573f83...
```

  **What the assertion was claiming.** Asset index 17 for 1.21.1 has two
  revisions on piston and they are both live: same 3,911 objects, same 449,557
  bytes, 142 `minecraft/lang/*` entries re-hashed between them. Mojang republished
  the index, Prism's mirror still names the older digest, and the edge GitHub's
  Windows runners reach named the newer one -- so a line asserting that two
  independent services agree about `sha1`, `url` and `size` was asserting Mojang's
  publishing schedule, which no launcher can keep. It is not flakiness in the
  pejorative sense: the value is stable on each service and moves when an upstream
  republishes, which makes it exactly the kind of claim this test file's own header
  tells its authors not to write.

  **What it asserts now.** The identity of the index (`id`), and for each side
  that its `url` names its own digest and id -- `.../packages/<sha1>/<id>.json`,
  with a 40-character digest -- which is the claim about *this* launcher: that it
  passes the index through without rebuilding it. The field-for-field equality is
  kept, but conditional on the two sides agreeing about a revision, which is the
  ordinary case and the one a local run sees today.

  **The receipt for the other branch is the next dispatch, and that is deliberate.**
  A divergent-revision run cannot be produced on demand from here, because this
  machine's path to both services currently resolves to the same older revision;
  faking it would mean a fixture, and a fixture is the thing live tests exist to
  replace. If the runner still sees `9b16298b...` when the next dispatch runs, that
  run is what says the tolerant branch works; if it does not, the branch is
  unexercised and stays that way in this record rather than being called covered.

  **The push, and the first fully green run in this document.** `a6bc372` went to
  both remotes: the PR run it opened is `36579945260` (`test` and `lint`,
  success), and the dispatch that carries all four jobs is `36579946549` --
  `Test workspace` success, `Lint` success, `Live services` success at **18
  passed / 0 failed / 0 ignored in 99.78 s**, and both `Build exe` targets success.
  That is the first run since G101 in which every job that was scheduled did the
  work and passed, and the first time either workflow has produced an artifact:
  msvc 5,483,966 bytes and gnu 5,553,430 bytes as uploaded, 13,682,688 and
  13,950,976 uncompressed, the first with `PalantirMC.exe 603eb275...` matching the
  sidecar written beside it.

  Which of this gate's two branches the runner took cannot be read from the log,
  because a passing assertion prints nothing: either its edge of piston agreed
  with the mirror this time, or it disagreed and the tolerant branch held. What
  the run does say is that the suite is green from a vantage point whose revision
  of asset index 17 differed from this machine's an hour earlier -- and that the
  disagreement is real is measured above, not assumed.
- [x] G122: the ledger the compiled locales are read through -- what each of the
  other 32 carries against English's 3,846 names, how much of it falls back to
  English, and which plural arms the language's own rule can never select
  CHECK: python tools/gen_locale.py --report
         python tools/gen_locale.py --check
         cargo test --workspace --all-targets --locked
         cargo clippy --workspace --all-targets --locked -- -D clippy::correctness
  EXPECT: 33 trees carrying 89,177 of a possible 126,918 locale-key pairs (70.3%)
          sparsest translation th-TH at 459 keys (11.9%), fullest de-CH at 3,792 (98.6%)
          37,741 pairs (29.7%) read English's sentence; 244 names are in all 32
          six locales carry an `one` arm their own CLDR rule can never select
          1008 passed; 0 failed; 18 ignored, and clippy exit 0 at 42 warnings
  EVIDENCE: the tool's own ledger, in full, and the transcripts of the other three:

```
$ python tools/gen_locale.py --report
tag        keys  cover fallback plural select number    # arms
ar-SA      1577  41.0%     2269     19      5      8   45 few(13), many(13), one(19), other(19), two(13), zero(2)
cs-CZ      2554  66.4%     1292     52      5     22   56 =0(1), few(28), many(3), one(52), other(52)
da-DK      1133  29.5%     2713     25      0     11   14 =0(1), one(25), other(25)
de-CH      3792  98.6%       54     79     10     30   57 =0(1), one(79), other(79)
de-DE      3792  98.6%       54     80     10     30   58 =0(1), one(80), other(80)
en-US      3846 100.0%        0     80     10     32   56 =0(1), one(80), other(80)
es-419     3775  98.2%       71     82     10     28   54 =0(1), one(82), other(82)
es-ES      3765  97.9%       81     81     10     31   56 =0(1), one(81), other(81)
fi-FI       515  13.4%     3331      2      0      2    2 one(2), other(2)
fil-PH     1122  29.2%     2724     30      1      4    6 =0(1), one(30), other(30)
fr-FR      3484  90.6%      362     74      9     30   48 =0(1), one(74), other(74)
he-IL      1082  28.1%     2764     18      0      2    6 one(18), other(18)
hu-HU      3784  98.4%       62     26     10     32   13 =0(1), one(25), other(26)
id-ID      1230  32.0%     2616     28      5      8    9 =0(1), one(3), other(28)
it-IT      3767  97.9%       79     57     10     34   45 =0(1), one(57), other(57)
ja-JP      2989  77.7%      857     33      8     31   21 =0(1), one(21), other(33)
ko-KR      2923  76.0%      923     57      9     28   23 =0(1), one(30), other(57)
ms-MY      2300  59.8%     1546     49      9     15   14 =0(1), other(49)
nl-NL      3654  95.0%      192     74     10     28   52 =0(1), one(74), other(74)
no-NO      1110  28.9%     2736     27      3     12   16 =0(1), one(27), other(27)
pl-PL      3785  98.4%       61     81     10     32   73 =0(1), few(56), many(9), one(80), other(81)
pt-BR      3792  98.6%       54     82     10     28   61 =0(10), one(82), other(82)
pt-PT      1275  33.2%     2571     27      4      5    8 =0(1), one(27), other(27)
ro-RO      1397  36.3%     2449     35      1     15   16 =0(1), one(35), other(35)
ru-RU      3753  97.6%       93     81     10     21  102 =0(1), =1(13), few(60), many(2), one(72), other(81)
sr-CS      2412  62.7%     1434     65     10     25   58 =0(1), few(23), one(65), other(65)
sv-SE      3302  85.9%      544     67      9     32   47 =0(1), one(66), other(67)
th-TH       459  11.9%     3387      2      0      1    1 other(2)
tr-TR      3445  89.6%      401     43      7     30   46 =0(1), one(43), other(43)
uk-UA      3772  98.1%       74     79     10     23  103 =0(1), =1(1), =2(1), few(67), many(38), one(78), other(79)
vi-VN      2050  53.3%     1796     54      9     17   14 =0(1), one(5), other(54)
zh-CN      3749  97.5%       97     70     10     32   28 =0(1), one(21), other(69)
zh-TW      3792  98.6%       54     75     10     32   26 =0(1), =1(2), one(3), other(75)

trees            33: the reference's own 32 offered codes, plus ar-SA, which its list comments out
keys             3,846 English names; the trees carry 89,177 of a possible 126,918 (70.3% translated)
coverage         over the translations: sparsest th-TH at 459 (11.9%), fullest de-CH at 3,792 (98.6%)
fallback pairs   37,741 of 126,918 locale-key pairs (29.7%) read English's sentence
names in all 32 244 of the 3,846; the other 3,602 are missing from at least one translation
value bytes      2,700,238 of translated text
index bytes      178,354 (a u16 per entry)

arms outside the language's CLDR category set (unselectable for every count):
  id-ID    one
  ja-JP    one
  ko-KR    one
  vi-VN    one
  zh-CN    one
  zh-TW    one

$ python tools/gen_locale.py --check
locale generation is byte-identical

$ cargo test --workspace --all-targets --locked
    177 passed; 0 failed  (palantir-core, lib)
      8 passed; 0 failed  (palantir-core, tests/compat.rs)
    537 passed; 0 failed  (palantir-desktop, bin)
      4 passed; 0 failed  (palantir-desktop/tests/native.rs)
     31 passed; 0 failed  (palantir-loader, lib)
    251 passed; 0 failed  (palantir-net, lib)
      0 passed; 0 failed; 18 ignored  (palantir-net, tests/live.rs)

$ cargo clippy --workspace --all-targets --locked -- -D clippy::correctness
exit 0, 42 warnings
```

  **What the numbers say, and what they do not.** 70.3% of the locale-key pairs this
  launcher could carry are translated, and the 29.7% that are not are not a rounding
  error: they are what a reader of Thai (11.9% covered) or Finnish (13.4%) sees, which
  is English with the occasional translated sentence in it. That is the honest shape
  of the reference's own corpus, measured against the reference's own English -- and
  it is a *coverage* number, not a quality claim: 3,792 keys carried says nothing
  about how well they are carried, and this gate does not read a translation.

  The comparison that matters is with what the reference does: it bundles English and
  **fetches** the other 32 from its CDN at runtime with `fallbackLocale: 'en-US'`, so
  a reader who is offline is back to English while a reader here is not. Compiling
  them in is what makes the coverage above real rather than conditional, and its cost
  is the 2,700,238 bytes of the tables plus 178,354 bytes of index in the binary.

  Three smaller things this ledger is the record for. **244 names are carried by all
  32 translations** and 3,602 are missing from at least one, which is why the
  fallback is per key and not per language: a partly translated tree is a mixture,
  and `locale::lookup` returning `None` for the untranslated half is what keeps the
  English path the generator's own code rather than a second renderer.

  **Six locales carry an `one` arm no rule of theirs can select** -- `id-ID`, `ja-JP`,
  `ko-KR`, `vi-VN`, `zh-CN`, `zh-TW` are languages whose CLDR category set is
  `other` alone, and their translations still say `one(...)` because the reference's
  own files do. That is dead in the reference too: `Intl.PluralRules('ja-JP')`
  answers `other` for every number, so the arm is unreachable on both sides. The
  generator reports it rather than refusing the tree, because a translator's
  `one` is not a defect -- it is a category the language does not have.

  **`ru-RU` and `uk-UA` are the two trees that use explicit arms** (`=0`, `=1`,
  sometimes `=2`) and the two heaviest users of `#` (102 and 103), which is what a
  language with `few`/`many` and an `11`-in-every-hundred rule looks like when it is
  translated carefully. `pt-BR` is the only tree with a two-digit `=0` count (10).

  **One correction to G120's own report.** Its table printed a `keys` column that was
  `leaves` a second time -- one leaf per key, so the two were always equal -- which
  meant the row said nothing about coverage, the one thing the ledger exists to say.
  The column is now the coverage share, the arm counts print as `=0(2)` rather than
  `=0=2`, and the totals gained the fallback-pair and all-32 counts above. The
  generated table is unchanged by all of it: `--check` still prints byte-identical.

  The runner could not be the receipt either: this slice's push, `19b44d2`, is run
  `36584261515`, the same block as the 90 before it -- `Lint` failed in 2 s
  (`14:39:10Z` to `14:39:12Z`) and `Test workspace` in 3 s (`14:39:10Z` to
  `14:39:13Z`), each with **zero steps**, on `The job was not started because recent
  account payments have failed or your spending limit needs to be increased`, with
  `Live services` and `Build exe` skipped rather than scheduled. That is the 91st
  consecutive failure counting from `36038333030`, and the branch's run list holds
  93 runs of which every one failed, so this slice's receipt is the local run
  transcribed above: the same compiler and the same flags on this same commit, but
  not the clean checkout `AGENTS.md` calls the authority, and nothing here should be
  read as "CI passed".

- [x] G115: what the interface costs at size -- the instance page's two listing
  tabs against a folder of five thousand, Discover against a hundred hits, and the
  interaction clock against every control on a page in flight -- and the one cost
  that was worse than linear, fixed
  CHECK: cargo test -p palantir-desktop --locked scale -- --nocapture
         cargo test -p palantir-desktop --locked
         cargo clippy -p palantir-desktop --all-targets --locked -- -D clippy::correctness
  EXPECT: the instance page's Files tab at 5,000 entries is 2,311 ms of directory
          read a frame and 2,304 ms of page, and both fall to 10.8 ms and 23.7 ms
          once the read stops looking every name up again (a 213x and a 97x cut)
          the Content tab is 46.8 ms a frame at 5,000 mods, 4.9 ms of it row-key
          interning; 500 mods is inside a 16.7 ms frame and 5,000 is not
          Discover is 3.3 ms at 100 hits and 26.3 ms at 1,000, which the API never
          returns in one page; the clock is 0.38 ms to tick 5,000 tweens in flight
          7 passed; 0 failed in the measurement, and the crate's own suite green
  EVIDENCE: the two runs and the fix between them, transcribed in full.

  The runner's own receipt of this commit is different from the local one and is
  worth separating: on the pull-request path `36599941647` ran `Test workspace` and
  `Lint` and skipped `Live services` and `Build exe` by its own rule, and a
  dispatch of the same commit, `36600372776`, ran all five -- **1011 passed / 0
  failed** and 18 ignored across the workspace suites (177 + 8 + 540 + 4 + 31 +
  251, the three measurement tests being the difference from G122's 1008), the live
  suite 18 passed / 0 failed in 109.66 s, and both Windows exes built and staged
  (msvc 5,483,872 B, gnu 5,553,494 B). The numbers below are this machine's; those
  are the runner's, on a shared VM with no warm scratch directory.

```
$ cargo test -p palantir-desktop --locked scale -- --nocapture     # before the fix
== instance page, per frame ==
mods/ read (store::content)        n=0          0.092 ms
Content tab view                   n=0          0.145 ms
Files tab view                     n=0          0.222 ms
mods/ read (store::content)        n=100        0.409 ms
Content tab view                   n=100        1.841 ms
Files tab view                     n=100        0.291 ms
mods/ read (store::content)        n=1000       3.188 ms
Content tab view                   n=1000      17.165 ms
Files tab view                     n=1000       0.540 ms
mods/ read (store::content)        n=5000      18.132 ms
Content tab view                   n=5000     110.361 ms
Files tab view                     n=5000       0.681 ms
```

The Files tab read a root of **one** entry for that whole column: the fixture put
its files in `mods/`, which is what the Content tab lists, so the flat 0.681 ms
was one row (`mods` itself) and said nothing about the tab. The fixture grew the
same count in the instance root, and the second run separated the tab's two halves:

```
$ cargo test -p palantir-desktop --locked scale -- --nocapture     # before the fix
Files tab view                     n=100       13.871 ms
  of which: files/ read            n=100       14.692 ms
  of which: N rows in one column   n=100        0.281 ms
Files tab view                     n=1000     217.688 ms
  of which: files/ read            n=1000     168.786 ms
  of which: N rows in one column   n=1000       3.020 ms
Files tab view                     n=5000    2304.338 ms
  of which: files/ read            n=5000    2311.148 ms
  of which: N rows in one column   n=5000      16.688 ms
```

So it was not the drawing: five thousand rows in one column cost 16.7 ms while the
*read* cost 2,311 ms. `store::files` called `path.is_dir()` and `fs::metadata(&path)`
on every entry -- two lookups by name of names the directory read had just returned
-- where `mods::list_mods` reads `DirEntry::file_type` and measured 12 ms at the same
count. The cost per entry grew with the folder (0.147 ms at a hundred, 0.169 ms at a
thousand, 0.462 ms at five thousand: 50 times the entries for 157 times the time),
and `files()` now takes both answers out of the scan, following a link only when the
entry *is* one, which is what the name-based calls did and what a Windows junction
in an instance folder needs.

```
$ cargo test -p palantir-desktop --locked scale -- --nocapture     # after the fix
mods/ read (store::content)        n=0          0.076 ms
  of which: row key interning      n=0          0.000 ms
Content tab view                   n=0          0.137 ms
Files tab view                     n=0          0.135 ms
  of which: files/ read            n=0          0.074 ms
  of which: N rows in one column   n=0          0.004 ms
mods/ read (store::content)        n=100        0.215 ms
  of which: row key interning      n=100        0.080 ms
Content tab view                   n=100        0.994 ms
Files tab view                     n=100        0.503 ms
  of which: files/ read            n=100        0.233 ms
  of which: N rows in one column   n=100        0.158 ms
mods/ read (store::content)        n=1000       1.473 ms
  of which: row key interning      n=1000       0.840 ms
Content tab view                   n=1000       9.035 ms
Files tab view                     n=1000       4.580 ms
  of which: files/ read            n=1000       1.785 ms
  of which: N rows in one column   n=1000       1.695 ms
mods/ read (store::content)        n=5000      18.531 ms
  of which: row key interning      n=5000       4.937 ms
Content tab view                   n=5000      46.837 ms
Files tab view                     n=5000      23.672 ms
  of which: files/ read            n=5000      10.830 ms
  of which: N rows in one column   n=5000       9.376 ms
== discover, per frame ==
results view                       n=20         0.737 ms
results view                       n=100        3.290 ms
results view                       n=1000      26.276 ms
== interaction clock, per frame ==
clock tick, all in flight          n=0          0.000 ms
N reads (one per control)          n=0          0.000 ms
N locked reads (ui::interaction)   n=0          0.000 ms
clock tick, all in flight          n=100        0.008 ms
N reads (one per control)          n=100        0.061 ms
N locked reads (ui::interaction)   n=100        0.019 ms
clock tick, all in flight          n=1000       0.077 ms
N reads (one per control)          n=1000       0.723 ms
N locked reads (ui::interaction)   n=1000       0.189 ms
clock tick, all in flight          n=5000       0.380 ms
N reads (one per control)          n=5000       3.967 ms
N locked reads (ui::interaction)   n=5000       0.914 ms
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 533 filtered out
```

  Three things this gate is careful not to claim.

  **The clock is not a cost, so it is not the finding.** It was the surface the plan
  expected to be expensive -- one lock and one hash lookup per control per frame --
  and at 5,000 controls that is 0.91 ms for the reads and 0.38 ms for the tick.
  Nothing here needs changing, and the row exists so nobody re-derives it later.

  **The Content tab's 46.8 ms is a measured limit, not a fixed defect.** It is 18.5 ms
  of directory read, 4.9 ms of row-key interning (`ui::scoped` formats a name and
  takes a process-wide mutex per row per frame, because a row's toggle is named from
  the file it acts on) and about 23 ms of building the cards. Roughly 9 us a row:
  **500 mods is 4.6 ms and inside a frame, 5,000 is not**, which is the number a
  reader should take away. It is linear, it is above the budget only at sizes a
  mods folder rarely reaches, and the page only pays it on the frames it draws --
  the shell asks for none while nothing is moving. Caching the listing and interning
  each row's key once, which is what Discover's `Load<Vec<Hit>>` already does, is the
  slice that would remove the 18.5 and the 4.9; it is not in this one, because it
  changes how the page is loaded rather than what it costs.

  **The numbers are this machine's, in a warm scratch directory.** The runner is a
  shared VM that runs this beside 533 other tests, so the reproduction is the shape
  (a read that is linear and a page that is linear in it) and the ratio between the
  two runs, not the millisecond. The three envelopes in `scale.rs` are set at
  roughly twice each measurement for that reason, and they guard a regression rather
  than restating the number.

- [x] G123: a texture the reader picks becomes the account's skin -- the launcher's
  first file dialog, the padding to the shape the service takes, the arm style read
  from the texture's own pixels, and the multipart body all of it arrives in
  CHECK: cargo test -p palantir-net --lib --locked
         cargo check -p palantir-desktop --locked --all-targets
         cargo test -p palantir-desktop --locked --bin PalantirMC
         cargo test --workspace --all-targets --locked
         cargo clippy --workspace --all-targets --locked -- -D clippy::correctness
         python tools/progress.py --check
         python tools/dashboard.py --check
  EXPECT: 255 passed, 0 failed in palantir-net's own suite, up from 251
          exit 0 for the type check of the desktop crate, with no warning in a line
             this slice added -- which is what compiles the Win32 call and the flow
          552 passed, 0 failed in the desktop binary's suite
          1027 passed, 0 failed, 18 ignored between the seven suites, which is the
             merged tree: this slice's commit sits on G115's, and G115's three
             `scale.rs` tests are what makes the desktop suite 552 rather than the
             549 this slice measured before the merge
          exit 0 for clippy, with no warning in a line this slice added
  EVIDENCE: the transcripts of these commands on this tree:

```
$ cargo test -p palantir-net --lib --locked
test auth::tests::the_upload_body_is_the_two_parts_the_service_reads ... ok
test auth::tests::the_file_name_in_the_part_is_a_name_and_cannot_end_the_header_line ... ok
test auth::tests::an_upload_posts_the_multipart_body_to_the_skin_service ... ok
test auth::tests::an_upload_with_no_texture_is_refused_before_any_request_is_made ... ok
test result: ok. 255 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 17.82s

$ cargo check -p palantir-desktop --locked --all-targets
    Checking palantir-desktop v0.1.0 (C:\palantirmc-jobs\crates\palantir-desktop)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1m 52s
$ echo $?
0

$ cargo test -p palantir-desktop --locked --bin PalantirMC
test pages::skins::tests::adding_a_skin_asks_the_shell_for_a_file_and_takes_one_at_a_time ... ok
test pages::skins::tests::an_upload_that_worked_reloads_the_lists_and_a_stale_answer_is_dropped ... ok
test pages::skins::tests::the_three_answers_to_that_request_draw_three_different_pages ... ok
test pick::tests::a_cancel_is_an_empty_buffer_and_a_choice_is_read_to_its_nul ... ok
test pick::tests::a_file_that_is_readable_is_read_and_one_over_the_ceiling_is_refused_by_size ... ok
test pick::tests::the_ceiling_is_the_one_the_message_names ... ok
test pick::tests::the_filter_is_one_entry_and_a_double_nul ... ok
test skin::tests::a_legacy_texture_is_padded_to_the_modern_shape_with_both_left_limbs_filled ... ok
test skin::tests::a_modern_texture_comes_back_pixel_for_pixel ... ok
test skin::tests::a_normalised_legacy_texture_draws_the_same_doll_as_the_legacy_one ... ok
test skin::tests::a_texture_of_neither_shape_is_refused_by_the_upload_path_too ... ok
test skin::tests::the_arm_style_is_read_from_the_two_columns_the_reference_reads ... ok
test skin::tests::the_left_limbs_are_the_right_ones_mirrored_rather_than_copied ... ok

test result: ok. 552 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 78.36s

$ cargo test --workspace --all-targets --locked
    177 passed; 0 failed  (palantir-core, lib)
      8 passed; 0 failed  (palantir-core, tests/compat.rs)
    552 passed; 0 failed  (palantir-desktop, bin)
      4 passed; 0 failed  (palantir-desktop/tests/native.rs)
     31 passed; 0 failed  (palantir-loader, lib)
    255 passed; 0 failed  (palantir-net, lib)
      0 passed; 0 failed; 18 ignored  (palantir-net, tests/live.rs)
exit 0 -> 1027 passed / 0 failed / 18 ignored

$ cargo clippy --workspace --all-targets --locked -- -D clippy::correctness
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 4m 32s
$ echo $?
0
$ grep -c '^warning: ' .scratch/g123-clippy-merged.log
42
$ diff <(grep '^warning: ' .scratch/g122-clippy-rebased.log | sort) \
       <(grep '^warning: ' .scratch/g123-clippy-merged.log | sort)
# nothing is printed: the 42 warnings on the merged tree are the same set this
# branch carried before the slice -- G115's own code added none -- and three
# warnings from this slice's first draft were fixed rather than shipped
# (`manual_char_comparison`, `type_complexity` twice)

$ python tools/progress.py --check
$ python tools/dashboard.py --check
CONFIRMED: the page carries all 120 gates, 6 stage cards and every subject as written
```

  **Every number above was re-measured after the rebase, and the merge is why.** This
  slice's commit sits on G115's, which landed while G123 was being written, so the
  branch was rebased before the push and three documents conflicted where both sides had
  edited the same lines: `GATES.md` keeps G115's entry and then this one, `NEXT_STEPS.md`'s
  stage-3 row took this slice's wording plus G115's cost clause, and `tools/progress.py`'s
  stage-3 line carries both ranges. Nothing in the transcripts is carried over from the
  tree the slice was written on -- the only count that moved is the desktop suite's,
  because G115's three `scale.rs` tests are in it (549 -> 552, and 1024 -> 1027 in the
  workspace).

  **What this slice is, and where it came from.** G106 built the Skins page's writing
  half and ended by naming what it deliberately did not: the reference's file upload,
  which is a dialog and a multipart body. G105 had already measured the one piece of
  it that is not plumbing -- the reference's own arm-style test,
  `helpers/skins.ts`'s `determineModelType`, which answers slim or classic from the
  two columns at (54, 20), 2 wide by 12 tall -- and left it *unclaimed* rather than
  spent. This slice takes both: the picker, the transform, the arm style and the body.

  **The picker is Win32, and that is a decision rather than an accident.** A file
  dialog is the one thing this launcher has never had: the import flow *scans* the
  places other launchers keep their instances in, which works because there is one
  right answer to find, and a skin is wherever the reader saved it. The obvious
  implementation is a crate (`rfd`), which would be the first new dependency of this
  rewrite and would need its licence checked and a line in
  `THIRD_PARTY_NOTICES.md`. `GetOpenFileNameW` needs one more *module* of a crate the
  window code already links (`windows-sys`, already in `Cargo.lock`, already in the
  binary, already noticed), so no new crate enters the build and no new licence
  question is opened; `cargo test --locked` passing is the proof that the lock file
  did not move (`git status` in this slice's commit shows `Cargo.lock` untouched). The
  cost is the pre-Vista dialog rather than the COM `IFileOpenDialog` Windows draws for
  its own apps, and it is written in the module rather than left to be noticed. The
  call is the one part of this slice that cannot be tested -- a modal OS window, and a
  test that opened one would be waiting for a mouse -- so everything around it is a
  function with no `cfg`: the filter (whose double NUL is the terminator, and a filter
  missing one is a dialog with no file types at all), the buffer read (which stops at
  the first NUL, and answers "cancelled" for the empty string the dialog leaves
  behind), and the ceiling.

  **The transform, and the test that justifies it.** The service takes one shape --
  64x64 -- and a reader's file may be a 64x32 texture from before 1.8. Padding is not
  decoration: the modern layout's left arm and left leg boxes have no legacy
  counterpart, so a padded canvas with them left transparent is a player with two
  right limbs. The fill is the right limb's own box copied *face by face* -- each of
  its four four-column faces mirrored within its own columns -- rather than one flip of
  all sixteen, which would put the right arm's right face where its front belongs.
  That reasoning is worth exactly as much as the test that holds it: the front view
  cut from a legacy texture (which reads the right limbs *through* `PARTS`' mirrored
  stand-ins) is asserted equal, pixel for pixel, to the front view cut from the
  normalised one -- so the drawing half and the upload half of this module cannot
  drift apart, and the direction of the mirror is pinned by a property rather than by
  a sentence. It is a stronger receipt than the four-colour arm test beside it, which
  names 10/20/30/40 in both directions.

  The reference reaches this transform through its plugin's `normalize_skin_texture`,
  whose Rust is not in this tree (G105). This launcher's name for it is
  `skin::prepare`, which is that transform **and** the arm-style read in one decode,
  and it is one public function rather than two for a reason worth naming: the decode
  is the expensive part, both answers come out of the same image, and a second public
  function that only this module's tests called would be dead code in the binary -- a
  warning this tree counts, and a sign that the function has no caller rather than
  that it is useful. The departure is the *shape* of the call, not the arithmetic.

  **The arm style, which is G105's unclaimed measurement.** `determineModelType` reads
  2x12 pixels at (54, 20) and calls any paint there classic. Read against the format:
  that rectangle is the last two columns of the right arm's **back** face. A classic
  arm's faces are four pixels wide, so those columns are skin; a slim arm's are three
  -- its box packs as right 40..43, front 44..46, left 47..50, back 51..53 -- so the
  box's last two columns are padding in one and paint in the other. The tests paint
  the whole arm *except* those two columns and get slim, paint one pixel of the twelve
  and get classic, and record the consequence that surprises a reader: the page's own
  test texture paints the arm's *front* face and is therefore slim, because the front
  is not what the reference reads. The same test also settles that the *format* does
  not decide the model -- a legacy 64x32 whose arm box is painted the way a real one's
  is (all sixteen columns) reads classic through the same two columns -- which is the
  one place the padding and the detector could have disagreed. One thing the reference
  does with this answer that this launcher does not is *keep* it: it calls the detector
  only for skins whose document says `UNKNOWN` and caches the result. Here the answer
  is used once, at the moment of upload, and never written down -- nothing in this
  launcher stores a texture, so there is nothing to be wrong about later (that store is
  G124's).

  **The body.** `palantir_net::skin_upload_body` builds the `multipart/form-data` body
  -- a `variant` field, then a `file` part with its own `image/png` content type, then
  the closing delimiter -- and the transport grows one method (`post_multipart`) that
  puts those bytes on the wire under the `Content-Type` the body itself names, so
  header and delimiters cannot disagree. The double grows `insert_multipart` and its
  own log (`uploads()`), kept apart from `bodies()` on purpose: a multipart body is
  bytes, and a test asking what an upload sent through a `String` would be checking a
  lossy copy of the thing it means to check. The private `Verb` enum changed shape for
  this: it carries the body now (`Post(json)`, `Put(json)`, `Upload(body)`, `Delete`),
  which is what stops a DELETE being handed an empty string it does not send. Two
  choices in the body are worth naming. The boundary is a **constant that is checked
  against the bytes it delimits** rather than a random string: RFC 2046 asks for a
  boundary that does not occur in the content, a random one satisfies that almost
  always and can never be asserted to -- every test would read "the same body,
  whatever the boundary turned out to be" -- while a constant plus a walk to the first
  numbered variant that is absent both satisfies it and lets the gate spell the whole
  body out. The walk is exercised rather than argued: the gate's own test feeds the
  builder a texture containing the constant and asserts that it moves to `…Boundary1`,
  and another containing both to `…Boundary2`. And the file part's name is reduced
  (`file_part_name`): the name comes from a file dialog, so a quote or a newline in it
  would end the header line early and leave the rest of the body as bytes the service
  would read as a part of its own; a name that reduces to nothing becomes `skin.png`,
  because a part with an empty filename is a request the service refuses and nothing
  this launcher could say afterwards would explain why.

  **The flow, and the split.** `Ask::AddSkin` is the third kind of ask: not a question
  and not a change a page can describe, because what the reader picks is a path and the
  bytes behind it are not a page's to hold -- a page has never read a file in this tree,
  for the same reason it has never held a token. So the page asks for the dialog and
  carries the round; the shell opens it **on the frame thread** (a dialog owned by a
  window is modal to that window's thread and to nothing else), reads the file and pads
  it there, and sends only the upload off the thread, through the same account and the
  same `Store::wear` a row's Apply uses -- the upload is one more `SkinChange`, so no
  store method was added for it. The answer comes back through one seam with three
  arms: **cancelled** (the press is over and nothing is said -- a reader who changed
  their mind did not fail at anything), **done** (the service's own sentence on
  failure; silence plus a reload on success, exactly as G106's writes behave, because
  what changed is the document), and **no picker** (a sentence for a build that has no
  dialog, which is not the reader's fault and not the file's). The header's Add is
  drawn unusable while any of this is in flight, which is the same one-write-at-a-time
  rule the rows follow and a stronger one here: a second press would be a second modal
  dialog behind the first.

  **What this slice does not prove.** There is no live upload and there cannot be: it
  would change the appearance of a real account, so no test does it, which is the same
  limit G104's read and G106's write carry. The dialog itself is not gated -- the
  module says which parts are and why the call cannot be -- so the `GetOpenFileNameW`
  invocation is reasoned from the API's own contract rather than exercised, and a
  wrong flag there would be a runtime surprise rather than a test failure. The bytes
  are the transform's receipt, not the service's: the service's own acceptance of a
  padded legacy texture is unverified, and the reference's plugin Rust is not in this
  tree (G105), so "the bytes this launcher sends" and "the bytes the reference sends"
  are not compared anywhere -- what is compared is this launcher's own two readings of
  the format. And the reference's *other* way in -- dropping a file on the page
  (`get_dragged_skin_data`) -- has no equivalent here: iced 0.12 delivers no file-drop
  event, so the picker is the only door, and that is a platform limit rather than an
  omission.

  **One correction to something already in the ledger.** `NEXT_STEPS.md`'s stage table
  carried a stray row after stage 3's -- a bare ` -- see "What stage 3 has landed so
  far" |` continuation line, left by a rebase and rendering as a table row with one
  cell -- and the desktop prose still said the Skins page deliberately had no file
  upload. Both are fixed in this slice's commit rather than left for a later reader to
  triage. A third, found by writing this entry: the arm-style test first asserted that
  a legacy texture reads *classic* because the format has no slim variant, and the run
  proved the assertion wrong in the useful direction -- the detector reads pixels, and
  the test texture paints only the arm's front face, so it answers slim. The test now
  asserts what its pixels say, and the painted-legacy case is a separate assertion in
  the same test. A test that failed and was corrected is worth more here than one that
  was never written.

  **The runner is the receipt this time.** `origin` still cannot schedule a job, so
  this slice's push went to the public mirror as well, and there the same workflow runs.
  `36606649491` is this commit through the *pull-request* path: `Test workspace` green in
  2m6s and `Lint` in 1m33s, with `Live services` and `Build exe` skipped rather than
  scheduled, which is that path's own rule. `36607101415` is the same commit dispatched
  through all five (`gh workflow run ci.yml --repo msedge-projects/PalantirMC --ref
  rewrite-modrinth-native`) and green in 7m38s: the live suite 18 passed / 0 failed in
  103.92s, and both Windows exes staged (msvc 5,496,795 B, gnu 5,564,353 B). The
  runner's workspace rows are 177 + 8 + 552 + 4 + 31 + 255 passed, 0 failed, 18 ignored --
  **1027, the same total this machine measured**, which is what makes the two receipts one
  claim rather than two numbers that happen to agree. Its desktop suite ran the same 552
  names in 9.45s against this machine's 78.36s, and that gap is the machine rather than
  the tests: this checkout was sharing a CPU with another agent's build at the time. The
  push to `origin` is
  `36606628779`, failed in 5s with zero steps and the same billing message as the 90
  before it -- recorded rather than counted.

- [x] G116: the instance page's listing is a load rather than a draw -- every tab's
  own folder is read once, off the frame thread, when the tab is entered, and the
  view draws a state it was handed instead of walking a directory
  CHECK: cargo test -p palantir-desktop --locked -- --nocapture
         cargo clippy -p palantir-desktop --all-targets --locked -- -D clippy::correctness
  EXPECT: the Content tab's frame at 5,000 mods falls from 46.8 ms to 32.0 ms and the
          Files tab's from 23.7 ms to 12.0 ms, because the 8.0 ms read and the 5.4 ms
          of row names are paid once per tab entry instead of per frame (Files: 8.9 ms)
          at 500 rows the page is about 3 ms and inside a 16.7 ms frame; at 5,000 it is
          32.0 ms and is not, which is the drawing of 5,000 cards and nothing else
          546 passed; 0 failed; 0 ignored, and clippy exit 0, adding no warning
  EVIDENCE: the table this slice's own measurement prints, and the arms of it that
  moved. The runner agrees with this machine where the two can be compared: run
  `36610530981` on `a7cc615` is 1033 passed / 0 failed and 18 ignored across the
  workspace suites, this slice's six tests among them, on a tree that also carries
  the Skins picker's own slice.

```
$ cargo test -p palantir-desktop --locked -- --nocapture          # after this slice
== instance page, per frame ==
Content: the read (once a tab)      n=0          0.130 ms
Content: the row names (once a tab) n=0          0.000 ms
Content tab view (listing loaded)   n=0          0.056 ms
Files tab view (listing loaded)     n=0          0.055 ms
Files: the read (once a tab)        n=0          0.110 ms
  of which: N rows in one column    n=0          0.004 ms
Content: the read (once a tab)      n=100        0.394 ms
Content: the row names (once a tab) n=100        0.167 ms
Content tab view (listing loaded)   n=100        1.130 ms
Files tab view (listing loaded)     n=100        0.450 ms
Files: the read (once a tab)        n=100        0.465 ms
  of which: N rows in one column    n=100        0.277 ms
Content: the read (once a tab)      n=1000       1.451 ms
Content: the row names (once a tab) n=1000       0.803 ms
Content tab view (listing loaded)   n=1000       7.728 ms
Files tab view (listing loaded)     n=1000       2.370 ms
Files: the read (once a tab)        n=1000       1.872 ms
  of which: N rows in one column    n=1000       1.728 ms
Content: the read (once a tab)      n=5000       7.985 ms
Content: the row names (once a tab) n=5000       5.408 ms
Content tab view (listing loaded)   n=5000      31.971 ms
Files tab view (listing loaded)     n=5000      12.034 ms
Files: the read (once a tab)        n=5000       8.947 ms
  of which: N rows in one column    n=5000       8.646 ms
test result: ok. 546 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

  **What the shape of the change is.** `pages::instance::Asked` is the page's
  request and `pages::Ask::Instance` is how the shell receives it; the shell resolves
  the instance id to a directory on the frame thread -- that is a field read, not a
  walk -- and makes the read in `store::off_thread`, the same worker Discover's
  search uses, and the answer comes back through
  `pages::Message::instance_result` as `instance::Message::Listed`. The page's round
  is the guard Discover's is: a read the reader has replaced, because they pressed
  another tab, is dropped rather than drawn under the new tab's heading. The listing
  is `store::Listing`, five variants rather than a `Vec<String>`, because a row is
  not a name: the Content tab draws an enabled state and a toggle per row, the Files
  tab a glyph and a size, the Worlds tab whether a world has ever been opened.

  **A tab owes its read rather than asking for it, which is why the round moves on
  the ask.** `State::opening` answers "idle, and this tab has a listing", and
  `Shell::opening_command` runs at the end of every update -- so a tab change marks
  the listing idle and the shell answers it on the *same* turn, with no signature
  change to `Screen::retarget`, the seam that deliberately reads no disk. The test
  for the stale answer had to be written against that ordering: a slow Content read
  can only arrive after the Files read has been asked for, so the test asks for Files
  first and delivers Content's answer into that. The first version of the test
  asserted the stale answer changed nothing while the page was still in the previous
  round, which cannot happen -- it failed, which is how the ordering got written
  down.

  **What is left here is drawing, and the honest number is where it crosses.** At
  5,000 mods the Content tab is 32.0 ms a frame, about 23 ms of it building 5,000
  cards and 9 ms the rows themselves; there is nothing left in the read path to
  remove. Roughly 6 us a row puts **500 mods at about 3 ms and 5,000 at 32.0 ms**, so
  the page is inside a frame for the sizes an instance actually reaches and is not at
  the size the plan named -- what would move that number is virtualizing the list,
  which changes what the tab draws rather than when it reads. The clock and Discover
  are unchanged by this slice and still measured: 0.46 ms to tick 5,000 tweens, 4.4 ms
  for a hundred result cards and 42.4 ms for a thousand, which the API never returns
  in one page.

  **The compiler is part of the receipt.** `body` and `listing_body` take no `Store`:
  the signature of the function that draws a tab has nothing in it that can read a
  disk, so the property cannot come back by accident the way it arrived. What a
  signature cannot say, the page's tests do: a tab is read once and a tab change asks
  again, a stale answer is dropped and the fresh one lands, a toggle asks for a fresh
  listing, an empty listing is `Load::Empty` rather than `Ready(vec![])` (so the empty
  arm draws the reference's own card), every loaded row keeps its own clock name
  across a reload, and all six tabs draw in all five load states under all four
  themes.

- [x] G117: the instance page's tab body is a window -- a frame draws the rows the
  scroll region says are on screen, and the rows it does not draw are two spacers
  holding their place
  CHECK: cargo test -p palantir-desktop --locked -- --nocapture
         cargo clippy -p palantir-desktop --all-targets --locked -- -D clippy::correctness
  EXPECT: the Content tab's frame at 5,000 mods falls from 32.0 ms (G116) to 0.139 ms
          and the Files tab's from 12.0 ms to 0.107 ms, because both draw a twenty-row
          window instead of the listing; the frame at 100 and the frame at 5,000 are one
          number, asserted as a ratio rather than described; a tab whose region has not
          reported yet draws the fallback window at 0.358 ms and 0.436 ms, bounded by a
          height rather than by the listing
          566 passed; 0 failed; 0 ignored, and clippy exit 0, adding no warning
  EVIDENCE: this slice's own table, unchanged in every row it does not name. Nothing
  under `store::` moves: the read was already one call per tab entry, and this slice
  is about what a frame does with the answer. `tests/native.rs`'s four tests -- the
  ones that read the tree for a browser or a scripting engine -- pass beside it.

  **The runner agrees with this machine.** Run `36675700351` on `d3acb83` is the
  pull-request path: `Lint` and `Test workspace` green in 1m55s, 1041 passed / 0
  failed and 18 ignored across the workspace suites (177 + 8 + 566 + 4 + 31 + 255),
  this slice's eight new tests among the 566, with `Live services` and `Build exe`
  skipped by that path's own rule. Run `36675919956` is the same commit through all
  five jobs: the same counts, the live suite 18 passed / 0 failed in 95.28s, and both
  Windows exes staged (msvc 5,500,023 B, gnu 5,568,247 B).

```
$ cargo test -p palantir-desktop --locked -- --nocapture          # after this slice
== instance page, per frame ==
mods/ read (store::content)        n=0          0.092 ms
Content: the read (once a tab)     n=0          0.087 ms
Content: the row names (once a tab) n=0          0.000 ms
Content view (no report yet)       n=0          0.047 ms
Content view (region reported)     n=0          0.047 ms
Files view (no report yet)         n=0          0.047 ms
Files view (region reported)       n=0          0.047 ms
  of which: rows drawn, reported   n=0              0 rows of 0
  of which: rows drawn, no report  n=0         0 /    0 rows of 0
Files: the read (once a tab)       n=0          0.099 ms
  of which: every row built (no window) n=0          0.004 ms
mods/ read (store::content)        n=100        0.220 ms
Content: the read (once a tab)     n=100        0.222 ms
Content: the row names (once a tab) n=100        0.081 ms
Content view (no report yet)       n=100        0.339 ms
Content view (region reported)     n=100        0.135 ms
Files view (no report yet)         n=100        0.259 ms
Files view (region reported)       n=100        0.105 ms
  of which: rows drawn, reported   n=100           20 rows of 100
  of which: rows drawn, no report  n=100      57 /  100 rows of 100
Files: the read (once a tab)       n=100        0.255 ms
  of which: every row built (no window) n=100        0.156 ms
mods/ read (store::content)        n=1000       1.512 ms
Content: the read (once a tab)     n=1000       1.635 ms
Content: the row names (once a tab) n=1000       0.836 ms
Content view (no report yet)       n=1000       0.342 ms
Content view (region reported)     n=1000       0.136 ms
Files view (no report yet)         n=1000       0.781 ms
Files view (region reported)       n=1000       0.194 ms
  of which: rows drawn, reported   n=1000          20 rows of 1000
  of which: rows drawn, no report  n=1000     57 /  177 rows of 1000
Files: the read (once a tab)       n=1000       3.250 ms
  of which: every row built (no window) n=1000       2.760 ms
mods/ read (store::content)        n=5000       7.893 ms
Content: the read (once a tab)     n=5000      10.107 ms
Content: the row names (once a tab) n=5000       4.938 ms
Content view (no report yet)       n=5000       0.358 ms
Content view (region reported)     n=5000       0.139 ms
Files view (no report yet)         n=5000       0.436 ms
Files view (region reported)       n=5000       0.107 ms
  of which: rows drawn, reported   n=5000          20 rows of 5000
  of which: rows drawn, no report  n=5000     57 /  177 rows of 5000
Files: the read (once a tab)       n=5000       8.599 ms
  of which: every row built (no window) n=5000       8.663 ms
test result: ok. 566 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

  **Where the rule comes from: the reference's own virtual scroll, ported.**
  `ui/src/composables/virtual-scroll.ts` computes a `visibleRange` from the scroll
  container's `scrollTop` and `clientHeight` against a caller's `itemHeight`, with
  `bufferSize = 5` rows either side and `initialItemCount = 20` before the container
  has been measured, and pads with `visibleTop = start * itemHeight` inside a
  container whose `minHeight` is the whole list's. Both tabs that can get long use
  it: `content-tab/components/ContentCardTable.vue` with `itemHeight: 74` and
  `files-tab/layout.vue` with `itemHeight: 61`. `crate::scroll::window` is that
  computation, as a pure function of `(len, row_height, Geometry)` so it is tested
  without a window, and `Geometry::of` is the one line that turns iced's own
  `scrollable::Viewport` into it. Two of the reference's properties are carried
  deliberately: a range is never *shorter* than the window, so at the end of a list
  it slides instead of shrinking (a shrinking range would make the last screenful
  the cheapest frame and the one before it the most expensive, for no visible
  reason), and an empty listing is `0..0` rather than a window's worth of nothing.

  **Where the port has to differ, and it is the only place it does.** The reference
  measures its scrollable ancestor on mount -- `watchEffect` reads `clientHeight`
  before any scroll -- which is why twenty items are enough for it: they are one
  frame's worth. iced publishes a scrollable's viewport only from an *event* (wheel,
  touch, a scrollbar drag, a key; `notify_on_scroll` in `iced_widget`'s
  `scrollable.rs`), and it declines to publish at all when the content fits, so a tab
  the reader has opened and not yet touched would draw twenty rows and then a band of
  nothing on any taller window -- 480px of a 24px-row Files listing. So the fallback
  is a *height* rather than a count, `INITIAL_VIEW = 4,000`px with
  `INITIAL_ROWS = 20` as its floor, and the price of it is measured rather than
  argued: 0.358 ms on the Content tab and 0.436 ms on Files, against 12.0 ms when the
  tab drew its whole listing, falling to 0.107 ms the moment the reader scrolls. A
  window taller than 4,000px of body would draw a band of nothing until its first
  event; that is a deliberate trade against drawing an unbounded number of rows on a
  screen nobody has, and it is the one number here that is an assumption.

  **The layout that makes it possible is the reference's too.** `instance/Layout.vue`
  has `renderMode: 'scroll'` (the whole page moves inside `.app-viewport`) and
  `'fixed'` (`shrink-0` header and tabs, `min-h-0 flex-1 overflow-y-auto` body), and a
  windowed listing is the second case: rows are placed at `index * row_height` inside
  a region whose own height is the number the window is computed from, and a page
  that scrolled as a whole could not name where its list starts without measuring
  everything drawn above it. Every row is therefore a *slot* of one height --
  `CONTENT_ROW = 86` (the toggle's 40, the card's two 16px paddings, its two
  hairlines, and the 12px gap the tab already put between two cards) and
  `PLAIN_ROW = 24` -- and the two spacers hold the extent the rows that are not drawn
  would have had, which is what keeps the scrollbar's size a property of the listing
  rather than of the window. One consequence is recorded rather than hidden: a row's
  label is drawn on one line (`Wrapping::None`), because a wrapped name is a row whose
  height depends on a string and the arithmetic above is only as good as its row
  height. The reference's own `itemHeight` makes the same trade.

  **What the frame costs now is a function of the window, and the test says so as a
  ratio.** `scale.rs` measures the same frame at 0, 100, 1,000 and 5,000 rows and
  asserts that the 1,000- and 5,000-row frames are within three times the 100-row one
  (they are the same number to within a tenth of a millisecond), which is a property a
  page that went back to drawing its whole listing fails by a factor of sixty. The
  fallback frames are compared with a ceiling of 4 ms instead, because their window is
  a fixed height of rows and at 24px that is more rows than a hundred are. The unwindowed
  cost is still printed beside them -- `every row built (no window)`, 8.663 ms at 5,000
  -- because a measurement of the alternative is what makes the slice's number mean
  something. `pages::instance`'s own tests cover the seam the numbers do not: the
  region's report is what moves the window, a tab change leaves the geometry where
  iced has it (its `Scrollable` keeps its offset across one, and a page that reset the
  window would draw the top of a list whose region is still scrolled down), and five
  thousand rows draw under all four themes in both scroll states.

  **What this does not do.** Discover's results list is not windowed: it is 2.1 ms at
  a hundred hits and 18.7 ms at a thousand in this run (3.3 and 30.8 in the same test
  an hour earlier, which is the shared runner rather than the page), and it is a
  smaller question than this one was because the API answers twenty results a page.
  The body's region is not scrolled to the top on a tab change -- iced keeps the
  offset, which is why the geometry is kept with it -- and no other page has a list
  long enough for this to be measured yet, which is a fact about those pages rather
  than a claim about them.

- [x] G118: this launcher does not hold a Modrinth credential -- the four account
  surfaces are dropped by decision, and each of them says which service it is
  waiting for instead of promising a slice
  CHECK: grep -rn "needs_account(" crates/palantir-desktop/src/ | grep -v "pub fn needs_account"
         cargo test -p palantir-desktop --locked
         cargo clippy -p palantir-desktop --all-targets --locked -- -D clippy::correctness
         python tools/progress.py
         python tools/dashboard.py --check
  EXPECT: seven call sites, on the four surfaces this launcher does not have: the
          Servers page's listing paragraph and its three actions, an instance's Share
          tab, the Skins page's store sections and the checklist's *Sign in to
          Modrinth* press -- each of them drawing *needs a Modrinth account, which
          this launcher does not have* where two of them used to draw *is not
          implemented yet*
          the plan's stage-3 open list falls from three gates to two, both of them
          third-party-free (an instance's settings modal, the Skins page's edit
          modal), stage 3 reads 29 met / 2 open and the program 97%, and the two
          document tools exit 0 at 123 gates
          566 passed; 0 failed; 0 ignored, clippy exit 0, and no warning in a file
          this slice touched
  EVIDENCE: the transcripts below, and the reason the decision needed no measurement
  of its own: G105, G109, G110 and G111 are the measurement, and this gate is what a
  reader does with it.

```
$ grep -rn "needs_account(" crates/palantir-desktop/src/ | grep -v "pub fn needs_account"
crates/palantir-desktop/src/pages/instance.rs:561:                .push(ui::paragraph(theme, &store::needs_account("Sharing an instance"))),
crates/palantir-desktop/src/pages/instance.rs:973:        let reason = store::needs_account("Sharing an instance");
crates/palantir-desktop/src/pages/servers.rs:70:            Message::NewServer => self.notice = Some(store::needs_account("Creating a server")),
crates/palantir-desktop/src/pages/servers.rs:71:            Message::ManageBilling => self.notice = Some(store::needs_account("Billing")),
crates/palantir-desktop/src/pages/servers.rs:72:            Message::Refresh => self.notice = Some(store::needs_account("The server listing")),
crates/palantir-desktop/src/pages/servers.rs:133:            .push(ui::paragraph(theme, &store::needs_account("The server listing"))),
crates/palantir-desktop/src/pages/skins.rs:433:                    iced::widget::text(crate::store::needs_account("The skin store"))
crates/palantir-desktop/src/shell.rs:1698:                        self.modrinth_note = Some(store::needs_account("Signing in to Modrinth"));
crates/palantir-desktop/src/shell.rs:7336:            Some(store::needs_account("Signing in to Modrinth").as_str())
crates/palantir-desktop/src/shell.rs:7388:            Some(store::needs_account("Signing in to Modrinth").as_str())

$ python tools/progress.py
stage  what                                                   met  open   done
    3  Pages, in the reference's order: instance pages f...    29     2    94%

overall: 97%  (the stage numbers above, weighted by what each stage holds)
slices:  64 of 66 met in stages 0-5; 59 more gates are the shell this rewrite replaces

open work, the plan's own list; the gate counts are this file's estimate:
  stage 3  The instance-settings page is not built.         1 gates
  stage 3  The Skins page's edit half is not built.         1 gates

$ python tools/dashboard.py --check
CONFIRMED: the page carries all 123 gates, 6 stage cards and every subject as written
$ echo $?
0

$ cargo test -p palantir-desktop --locked
test result: ok. 566 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

$ cargo clippy -p palantir-desktop --all-targets --locked -- -D clippy::correctness
exit 0; `palantir-desktop` (bin "PalantirMC") 3 warnings and its test build 21,
none of them pointing at shell.rs, store.rs or the four page modules this slice
touched, and `grep -cE "never (used|read|constructed)"` is 0
```

  **What the four surfaces are, and why they are one decision rather than four.**
  Modrinth Hosting (the Servers rail slot, its listing and its billing page), an
  instance's Share tab (`shared-instances.modrinth.com`), the skin store beside the
  account's own skins, and the signed-in half of the panel's friends list plus the
  fundraising banner that sits near it. Every one of them is Modrinth's *account*
  service: the page data behind the first is Archon's 67 methods and Labrinth's
  `billing_internal.getProducts()`, the Share tab is an invite flow that belongs to a
  Modrinth identity, the store is a Modrinth table of skins, and the friends list is
  four `plugin:friends` calls. They arrive as one decision because the cost is one
  thing: a credential. G109 measured what that costs -- a secret in this launcher's
  own home directory (`PalantirPaths::home`, beside `prefs`; not Prism's
  `accounts.json`), plaintext unless OS protection is put on top, and every request
  made in the reader's name -- and G111 read the client's own `AuthFeature`, which
  stamps that one token onto Labrinth *and* Archon. A custom Minecraft launcher's job
  is the game: instances, versions, loaders, assets, launches. Its reader is owed a
  credential only if the launcher is going to use it, and these four surfaces are
  features of somebody else's product.

  **What is not dropped, and that line is the whole reason this gate is not
  `rm -rf modrinth`.** Everything this launcher already reads from Modrinth is the
  *published, anonymous* API: Discover's search, a project's document and its version
  list, the news feed in the panel, and the mod and pack installs built on all of
  those. None of it needs an account, all of it is measured and green in the live
  suite, and the decision above would not touch it if the account half had been taken
  instead -- which is what makes the cut a scope decision rather than a retreat from
  the reference.

  **What changed in the tree is a sentence, and the sentence is the point.**
  `store::not_implemented` said *is not implemented yet*, which is a promise that a
  later slice keeps; `store::needs_account` says *needs a Modrinth account, which this
  launcher does not have*, which is a fact. Seven call sites moved: the Servers page's
  listing paragraph and its three actions (`NewServer`, `ManageBilling`, `Refresh`),
  an instance's Share tab, the Skins page's store sections, and the checklist's *Sign
  in to Modrinth* press. Two of them were already reachable as text in the reference's
  own words and stay that way; the rest are this launcher's own copy, and the Servers
  page's test was rewritten from "says which stage does it" to "says the service it
  needs" so that a notice reading *is not implemented yet* fails rather than looks
  fine. The four surfaces keep their places in the reference's navigation -- the rail
  slot, the tab, the store cards, the checklist step -- because a reader who goes
  looking for Modrinth Hosting is owed the answer where they looked for it.

  **What that does to the ledger.** The plan's own open list for stage 3 held three
  gates, all of them behind a credential. Two of those bullets moved into the prose as
  a decision, and what is left is the work that needs no third party: an instance's
  settings modal (the write side of the settings model `model.rs` inherited, which has
  never had a control) and the Skins page's edit modal (reorder the account's own
  skins, take one off, keep the uploaded texture -- G123 landed the picker, the pad to
  64x64, the arm style and the multipart body). `tools/progress.py`'s `OPEN` moved with
  it in the same commit, which is the tool's whole reason for reading the plan instead
  of being told: stage 3 goes from 28 met / 3 open (90%) to **29 met / 2 open (94%)**,
  and the program from 96% to **97%** -- the gate itself counts, and two gates stopped
  being owed. A reader who disagrees with the decision has the measurements to argue
  with; G109's and G111's numbers are unchanged above, and the plan keeps the smallest
  first slice in writing too, because this is a scope decision and not a closed door.

  **The runner agrees with this machine.** Run `36679862832` on `fcdc935`, which
  carries this decision and the hours the plan is now read in, is the pull-request
  path: `Lint` and `Test workspace` green in 3m31s, 1041 passed / 0 failed and 18
  ignored across the workspace suites (177 + 8 + 566 + 4 + 31 + 255), with `Live
  services` and `Build exe` skipped by that path's own rule. The 566 is the desktop
  crate's own count, unchanged by a slice that moved copy, so the runner and this
  machine disagree about nothing here.

- [x] G124: the skins the reader adds are kept, and one can be edited -- arm style,
  cape, the Ears notice, and the way to take a skin off
  CHECK: cargo check -p palantir-desktop --locked --all-targets
         cargo test -p palantir-desktop --locked --bin PalantirMC
         cargo test --workspace --all-targets --locked
         cargo clippy --workspace --all-targets --locked -- -D clippy::correctness
         python tools/progress.py --check
         python tools/dashboard.py --check
  EXPECT: exit 0 for the type check of the desktop crate, with no warning at all in the
             binary's own build -- the three dead-code warnings the first draft drew are
             resolved rather than tolerated
          585 passed, 0 failed in the desktop binary's suite on the merged tree -- 552
             before this slice, 566 once the two slices already on the branch are counted
             (G116-G117), and this slice's 19 on top: the store's 7, `skin`'s 3 Ears
             tests, `text`'s 1 placeholder test, the page's 6 and the shell's 2
          1060 passed, 0 failed, 18 ignored between the seven suites
          exit 0 for clippy, the warning set still G123's own 42 lines -- the six new
             style warnings this slice's first draft drew were cleared rather than added
          exit 0 for both document checks, with the dashboard reading 124 gates
  EVIDENCE: the transcripts of these commands on this tree:

```
$ cargo check -p palantir-desktop --locked --all-targets
    Checking palantir-desktop v0.1.0 (C:\palantirmc-jobs\crates\palantir-desktop)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 42.74s

$ cargo test -p palantir-desktop --locked --bin PalantirMC
test pages::skins::tests::opening_the_editor_reads_the_row_it_was_pressed_on ... ok
test pages::skins::tests::each_of_the_editors_three_actions_travels_as_one_ask ... ok
test pages::skins::tests::the_editors_answer_closes_it_and_a_success_reloads ... ok
test pages::skins::tests::the_ears_notices_link_opens_the_mods_own_project ... ok
test pages::skins::tests::the_editor_draws_in_every_theme_and_around_both_answers_to_ears ... ok
test saved_skins::tests::adding_the_same_pixels_twice_is_one_row_and_brings_it_to_the_front ... ok
test saved_skins::tests::forgetting_removes_the_row_and_its_pixels ... ok
test saved_skins::tests::what_the_modal_writes_is_what_the_next_read_draws ... ok
test skin::tests::the_ears_marker_is_the_pixel_and_the_two_numbers_the_reference_reads ... ok
test skin::tests::a_legacy_texture_has_no_marker_because_its_canvas_has_no_row_32 ... ok
test shell::tests::the_stored_skins_are_read_back_in_the_readers_order_and_with_their_marker ... ok
test shell::tests::the_editors_press_is_an_ask_and_its_close_comes_back_to_the_page ... ok
test text::tests::a_placeholder_is_split_out_of_the_sentence_around_it ... ok
test result: ok. 585 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 107.73s

$ cargo test --workspace --all-targets --locked
test result: ok. 177 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.41s
test result: ok. 8 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.64s
test result: ok. 585 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 53.18s
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 1.06s
test result: ok. 31 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 3.77s
test result: ok. 255 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 5.55s
test result: ok. 0 passed; 0 failed; 18 ignored; 0 measured; 0 filtered out; finished in 0.00s

$ cargo clippy --workspace --all-targets --locked -- -D clippy::correctness
warning: `palantir-desktop` (bin "PalantirMC" test) generated 21 warnings (1 duplicate) (run `cargo clippy --fix --bin "PalantirMC" -p palantir-desktop --tests -- -D clippy::correctness` to apply 12 suggestions)
warning: `palantir-desktop` (bin "PalantirMC") generated 3 warnings (run `cargo clippy --fix --bin "PalantirMC" -p palantir-desktop -- -D clippy::correctness` to apply 1 suggestion)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 3m 55s

$ python tools/progress.py --check
$ python tools/dashboard.py --check
CONFIRMED: the page carries all 124 gates, 6 stage cards and every subject as written
```

  So far the Skins page has read the account's own skins and capes (G104), put one
  on or taken a cape off (G106), and uploaded a file the reader picked (G123). None of
  those kept anything: G123's own entry says it out loud -- "nothing in this launcher
  stores a texture, so there is nothing to be wrong about later (that store is
  G124's)". This slice is that store, and the editor the reference draws a row's
  changes in.

  **The store is a folder under the product's own directory.** `crate::saved_skins`
  keeps `<home>/skins/index.json` and one `<key>.png` per row, where `home` is
  `PalantirPaths::home` -- the directory this launcher's preferences already live in,
  and deliberately not the data root, because the data root can be an install another
  launcher created and can move. A row is keyed by the SHA-1 of its pixels, which is
  the digest the rest of this tree already speaks (Mojang publishes one per library,
  Modrinth one per file, and both are checked with it), and the index's `order` list is
  the reader's own order, which `add` maintains by promoting what was just added to the
  front -- the reference's own behaviour for a newly added skin. Writes go through
  `palantir_core::util::atomic_write`, so a crash mid-save cannot leave a half index;
  an index that will not parse is left in place and read as empty, which is `prefs`'
  rule for the same reason.

  **What the reference keeps and this does not, named rather than implied.**
  `helpers/skins.ts` labels every row with a `source` of `'default' | 'custom' |
  'custom_external'`; this enum has two, and the missing one is the point:
  `'default'` is a skin Minecraft ships, which the reference's plugin holds a copy of
  for its bundles, and this launcher has no bundle to hold. A row here is always the
  reader's own -- `Custom` for a file the picker returned, `CustomExternal` for a
  texture the account already owned and this launcher kept so it could be edited --
  and `CustomExternal` has no importer yet. `set_custom_skin_order` is `reorder`,
  which is tested and has no drag-and-drop list to call it from; `remove_custom_skin`
  is `forget`, which the editor calls. Those two -- and `Source::as_str`, the
  reference's own word for a source -- are marked `#[allow(dead_code)]` with the reason
  beside each, which is `locale.rs`'s `Direction` arrangement: the vocabulary is kept
  with its tests so the slice that paints the list does not re-derive it, and the
  attribute is what keeps the warning count honest about it.

  **Adding a skin now keeps a copy.** G123 uploaded a file and kept nothing; `keep_picked`
  writes the padded 64x64 texture, the file's own name with its extension dropped, the
  arm style the pixels were read for and `Source::Custom`, after the upload succeeds.
  It is best-effort on purpose: a store this launcher cannot write is not a reason to
  call the service's upload a failure. Adding the same pixels twice is one row rather
  than two -- `add` dedupes by digest, promotes the existing row and takes the new name
  and arm style, without writing a second file.

  **The page's read is one turn with two halves.** `Shell::skins` already read the
  account's appearance on a worker thread; `stored_rows` now reads the index and
  decodes each stored PNG on that same thread, and the page is handed
  `Loaded { appearance, saved }` so its rows and its doll cannot be a frame apart. A
  row's Ears answer is the marker in its *own* pixels (`skin::ears_of`: the RGB of
  pixel `(0, 32)` against the format's two magics, which the reference reads in
  `use-ears-mod-features.ts`), and a row whose pixels will not come back is still a
  row: the name and the arm style live in the index, and only the marker is the
  pixels' answer. The shell's test asserts exactly that, with a row whose texture is
  the bytes `this is not a PNG`.

  **The editor is the page's; the frame is the shell's.** The reference's
  `EditSkinModal.vue` is open when a row is pressed, and this launcher's is drawn from
  `skins::State::edit` through `pages::Screen::skins_edit`, with the shell's own
  `dialog` and a new `scrim` helper around the body and every press mapped back through
  `Message::Screen`. It is deliberately *not* a `Modal` variant: that would be a second
  copy of the page's editor to keep in step, and the two would drift the first time one
  was updated without the other. `Message::CloseModal` -- the scrim, and the dialog's
  own X -- now forwards `CloseEdit` to the page as well, so the editor is dismissed the
  way the shell's own modals are.

  **The three actions, and one departure from the reference that has to be written
  down.** Save writes the row's arm style and cape and puts the skin on, reading the
  texture back out of the store (`SkinChange::Upload`, the same one G123 sends). Forget
  removes the row and its pixels, needs no token, and works for a reader who is signed
  out. Take off is `SkinChange::NoSkin` -- the change G106 implemented and left
  unreachable, "because the reference reaches it from its edit modal".

  That last clause is in the job's own brief, and **the vendored source does not back
  it**. `helpers/skins.ts` defines `unequip_skin()` at line 177 and nothing in
  `app-frontend` calls it; `grep -rn unequip_skin vendor/modrinth-app/app-frontend`
  finds the definition and no caller at all. `EditSkinModal.vue` has Save and Cancel
  and no third action. Deletion is not in that modal either: `Skins.vue`'s
  `deleteSkin` is a button on its preview panel leading to a confirm dialog of its own
  (`delete-modal.title` / `delete-modal.description`). This page has no preview panel
  -- the doll above draws what Minecraft says is in force, not a candidate -- so its
  editor carries both, and that is this launcher's arrangement rather than the
  reference's. Two consequences follow. The take-off button's words are this
  launcher's own ("Take it off", a hand-written string, where the reference has no
  string for a control it never draws; the page's Saved-skins empty state and the note
  under the doll are the same kind of thing). And the deletion here has no confirm
  step, where the reference asks twice.

  **The Ears notice is the reference's sentence with its own link.** The modal draws it
  only when the row's texture carries one of the format's two magics.
  `app.skins.ears-feature-notice` is `"This skin uses features from the {ears} mod"`,
  and the reference fills the placeholder with a sentinel, splits on it and draws what
  is between the halves as a `router-link` to `/project/mfzaZK3Z` labelled "Ears".
  `text::placeholder_parts` is that split and `Message::OpenEars` is that link, sent
  through the page's own `Ask::Open(Open::Project(...))` to the project page this
  launcher already has. It is not the reference's whole Ears support: the mod's reader
  parses a feature request out of the same pixels and renders wings, ears and tails,
  and this launcher reads only that the marker is there and says so.

  **Two corrections this slice made in passing, because they were in lines it had
  already touched.** `text::placeholder` collided with the module's own private ICU
  `placeholder`; it is `placeholder_parts` now, and its test with it. And
  `skin::front_view` went dead the moment `Appearance::of` moved to `cut`: its only
  remaining callers were the tests, which is the case `prepare`'s own doc calls out as
  "a sign that the function has no caller rather than that it is useful", so it is gone
  and the tests take a cut's front out through a local helper. The six new clippy
  warnings the first draft drew (three hex literals whose digit groups clashed, three
  `Default::default()` plus assignment) are cleared rather than tolerated, which is why
  clippy's warning set is G123's own 42 lines exactly.

  **What this does not prove.** There is no live upload and no live unequip, and there
  cannot be: both change the appearance of a real account, so no test performs one,
  which is the limit G104's read, G106's write and G123's upload all carry. The store
  is exercised over a real temporary directory -- writing, reading back, reordering,
  forgetting, and a broken index -- but only this launcher's own reading of it: the
  reference's `minecraft-skins` plugin is not in this tree, so the two on-disk formats
  are not compared anywhere. And the editor is drawn and its asks are routed in tests,
  but what its buttons *do* to a real account is the same thing G106's rows do: a
  request this launcher's client already builds.

  **The runner agrees with this machine.** Run `36681338042` on `52da5df` is this
  slice's pull-request path: `Lint` and `Test workspace` green in 1m39s and 2m07s
  -- 1060 passed / 0 failed and 18 ignored across the workspace suites (177 + 8 +
  585 + 4 + 31 + 255), this slice's 19 among the desktop crate's 585 -- with
  `Live services` and `Build exe` skipped by that path's own rule. Run `36681579461`
  is the same commit through all five jobs: the same 1060 passed / 0 failed and 18
  ignored, the live suite 18 passed / 0 failed in 109.71s, both Windows exes staged
  (msvc 5,514,809 B, gnu 5,585,298 B), and clippy's 42 warning lines the same set
  this machine printed, because the two `Lint` jobs run the same
  `cargo clippy --workspace --all-targets --locked` command.

- [x] G125: an instance's own settings have controls -- the heap, the Java path
  and the JVM arguments, each behind the reference's own override switch, read back
  from the instance file and written on save
  CHECK: cargo test -p palantir-desktop --locked
         cargo clippy -p palantir-desktop --all-targets --locked -- -D clippy::correctness
         python tools/progress.py
         python tools/dashboard.py --check
  EXPECT: the instance page's gear reports `Ask::InstanceSettings(id)` rather than
          opening the modal itself, and the shell opens it from the store's own
          read, so an instance that overrides nothing shows the launcher's numbers
          with its switches off
          Save writes through `Instance::save`, and a switch turned off *removes*
          `OverrideJavaLocation`, `OverrideMemory` and `OverrideJavaArgs` rather
          than writing zeroes: off is "use the launcher's own setting", which is
          what the read that filled the form meant by it
          a heap under 128 MiB or a maximum below its minimum is the store's
          refusal, and the modal stays up with the sentence in it
          594 passed; 0 failed; 0 ignored, and clippy exit 0 with no new warning in
          the files this slice touched
          the instance-settings item on the plan's stage-3 open list narrows to the
          installation tab of the same modal, whose values come from a service --
          stage 3 reads 31 met / 2 open (94%), the other open item being the Skins
          page's edit half -- and both document tools exit 0 at 125 gates
  EVIDENCE: the transcripts below, and the three reasons the form differs from the
  reference's, each of them recorded where it is drawn rather than assumed.

```
$ cargo test -p palantir-desktop --locked
running 594 tests
test result: ok. 594 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 176.45s
running 4 tests
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.21s

$ cargo clippy -p palantir-desktop --all-targets --locked -- -D clippy::correctness
exit 0; `palantir-desktop` (bin "PalantirMC") 3 warnings and its test build 21,
the counts G118 recorded and G124 carried, and none of them pointing at
instance_settings.rs, ui.rs, store.rs, shell.rs or the page module this slice
touched -- so the 24 lines are the same set, and the slice added none

$ python tools/progress.py
stage 3  Pages, in the reference's order: instance pages f...    31     2    94%

overall: 97%  (the stage numbers above, weighted by what each stage holds)
slices:  66 of 68 met in stages 0-5; 59 more gates are the shell this rewrite replaces

$ python tools/dashboard.py --check
CONFIRMED: the page carries all 125 gates, 6 stage cards and every subject as written
```

  **The runner agrees with this machine.** Run `36686962535` on `7340321` is this
  slice's pull-request path: `Lint` and `Test workspace` green in 1m36s and 1m59s --
  1069 passed / 0 failed and 18 ignored across the workspace suites (177 + 8 + 594 +
  4 + 31 + 255), this slice's nine among the desktop crate's 594 -- with `Live
  services` and `Build exe` skipped by that path's own rule. The run's one red
  annotation is the lint job's advisory `cargo fmt --check` step, which `ci.yml`
  marks `continue-on-error`: it prints `Process completed with exit code 1` beside a
  job that concluded success, and it is a note rather than a gate.

  **What this is, and where the reference keeps it.** `model.rs` inherited a settings
  *reader* from `palantir-gui` -- the override-gate semantics Prism keeps
  (`JavaPath`/`OverrideJavaLocation`, `JvmArgs`/`OverrideJavaArgs`,
  `MinMemAlloc`/`MaxMemAlloc`/`OverrideMemory`) -- and the note it left behind says
  the write side belongs to the page that never arrived. The reference keeps it in
  `InstanceSettingsModal`, whose Java tab is
  `components/settings-modal/java-settings.vue`: a `Toggle` and its controls per
  setting, writing on every change. This slice is that tab as a modal of this
  shell's own: `crate::instance_settings` is the form, `store::InstanceSettings` is
  the value it reads and writes, and the instance page's header grows the reference's
  gear, which reports the instance rather than opening anything.

  **The three deliberate differences, and the price of each.** The reference writes
  on every change (`watch` on each ref); this form has a Save button, because the
  write here is a file write through `Instance::save` and a form that wrote per
  keystroke would write a half-typed heap -- `20` on the way to `2048`, which the
  store's own floor would refuse -- so the controls are buffers, and Save is where
  they become a value. The reference's memory control is a slider over the machine's
  RAM, and this kit has no slider: the two numbers the instance file actually holds
  are edited as fields, which is also what Prism's own pane shows, so the form edits
  the thing it writes rather than a shape of it. And the reference has no Save, no
  fields and no labels -- its own copy names the tab and the override, not the
  minimum and the maximum -- so the two field labels are this module's words, saying
  `MiB` because Prism's keys are megabytes by name and mebibytes by value.

  **The switch is the reference's `Toggle`, drawn rather than imported**
  (`ui::switch`): a 48x24 track, `bg-brand` under a 16px knob when on and
  `--button-bg` when off, the knob `bg-black/90` on -- a literal rather than a
  token, because the reference paints it with one on both themes it travels across --
  inset 4px from the track's edge, over a `--surface-5` hairline. What is *not*
  drawn is the knob's 200ms travel: the reference animates it with a CSS transition
  and this kit's clock carries hover tweens, so the knob is placed where the switch
  is rather than tweened across. A press is one frame's difference, and that is
  recorded here instead of implied.

  **What is not this half, and what is not a gate.** The installation tab beside the
  Java one -- game version, loader, loader build -- reads a service and its write is
  an install, so it stays the plan's open item, named in
  `crate::instance_settings`'s module doc and in the plan's own list. The reference's
  sync-override and sharing tabs are the Modrinth account surfaces G118 dropped. And
  the read the modal shows is the store's `instance_settings`, which is the *values
  in force*: an instance that overrides nothing draws the launcher's own prefs with
  the switch off, because that is what a launch would use.
- [x] G126: the Forge-shaped loaders install at launch preparation -- their own
  processors run before the resolve, and a machine with no Java is told so
  rather than discovered halfway through a chain
  CHECK: cargo test -p palantir-desktop --locked
         cargo test --workspace --all-targets --locked
         cargo clippy --workspace --all-targets --locked -- -D clippy::correctness
         python tools/progress.py --check
         python tools/dashboard.py --check
  EXPECT: three new desktop tests
          `cargo test --workspace` reports 1060 + 3 passed, 0 failed, 18 ignored
          clippy exit 0 at 42 warning lines, G124's own count unchanged
          both document tools exit 0, the ledger now carrying 126 gates
  EVIDENCE: G100 landed the two Forge-shaped loaders' processors and named its
            own gap: nothing outside `engine::forge` called `install` or built an
            `InstallCtx`, so the processors were *unreached from the interface*.
            This slice is the seam that reaches them, and it is deliberately the
            first half of the pair G107 measured -- the install at launch
            preparation, before the routing flip that follows it.

  **Where it runs, and why that order is not a preference.** `prepare_launch`
  gains one call before its resolve: `install::install_loader`, guarded by
  `install::loader_component(&profile).is_some()` so a vanilla or Fabric launch
  neither looks for Java nor touches the network. The processors patch the
  client jar and unpack the launcher stack, and the profile a launch resolves
  for `net.minecraftforge`/`net.neoforged` is the installer's own -- whose
  libraries only the processors produce -- so a resolve that ran first would
  resolve a profile nothing has installed. `loader_component` reads the build
  off the component's own version and the game off `net.minecraft`; a component
  with no build, or a profile with no game, is `None` rather than half a
  question asked of a maven.

  **The client jar is Mojang's, and it is not re-downloaded under another
  name.** `install_loader` reads Mojang's version file through `piston` and puts
  the client jar in the content store under the digest the manifest itself
  publishes (the `minecraft_jar` contract `InstallCtx` already states), which is
  the same jar `install::plan` fetches to `libraries/com/mojang/minecraft/...`
  for the classpath. The installer and every tool it names travel through that
  store too, under the `.sha1` sidecars their maven serves, so a stalled host
  resumes rather than restarts. The installer jar and its extracted `/data/...`
  files live under `cache/installers/`, keyed by loader, game and build: one
  build's installer is the same bytes for every instance that runs it.

  **The resume test is the installer's own.** A processor whose every declared
  output is already present and matching is skipped by `engine::forge::install`,
  which is what makes a second launch of the same build cheap and an interrupted
  install continue instead of restarting. The offline test states it where
  `prepare_launch` reaches it: a scripted installer naming one processor whose
  output is already on disk and whose digest matches, a stand-in `java` that is
  a file and nothing more, and the outcome `Installed { ran: 0, skipped: 1 }` --
  no maven coordinate resolved and no process started, which is the proof that
  the branch was not taken rather than a description of it. With no Java the
  call is `NoJava` *before the first request*; `prepare_launch` turns that into a
  blocked launch with the reason said, because a Forge instance cannot be
  installed without a runtime.

  **What this does not prove.** There is no live run of a Forge build through
  `prepare_launch` here, and there cannot be one on this machine: the processors
  download tens of megabytes and run Java tools for minutes, and the live proof
  of that sequence is G100's own `#[ignore]`d test against a real Forge and a
  real NeoForge build. What is exercised offline is the *wiring* -- which
  component is read as a loader, that no request is made when no Java is found,
  and that a fully-resumed install ends as `Installed { ran: 0, skipped: 1 }`.
  The resolve still answers `net.minecraftforge`/`net.neoforged` from the
  mirror: the flip is G127, and this slice deliberately leaves it where it was.

  **The id shifted by one.** The work order named this slice G125, but the
  instance-settings modal landed as G125 first, so this install is G126 and the
  routing flip behind it is G127.

  **The runner agrees with this machine, and here it had to: this slice has
  never been compiled anywhere else.** The pull-request path is run
  `36687632320` on `e76832c` -- `Lint` and `Test workspace` green in 1m30s and
  2m12s, 1073 passed / 0 failed and 18 ignored across the workspace suites (177
  + 8 + 598 + 4 + 31 + 255), with `Live services` and `Build exe` skipped by
  that path's own rule. Run `36688232327` is the same commit through all five
  jobs, green in 7m11s: `Test workspace` 2m8s and `Lint` 1m28s on the same rows,
  the live suite 18 passed / 0 failed in 92.16s, and both Windows exes staged --
  13,827,072 B (msvc) and 14,098,432 B (gnu). Those are the exe's own bytes,
  read back from the downloaded artifacts and matching the `.sha256` the
  workflow wrote beside each; the *zips* the same jobs upload are the ~5.5 MB
  figures the run table's earlier rows quote in the same slot, so the two look
  like a disagreement and are not one. `check_exe.py` reports the msvc and gnu
  exes both `needs nothing beyond Windows' own DLLs`, and clippy's 42 warning
  lines are the same set G124 recorded, none of them pointing at `install.rs`,
  `launch.rs` or `wire.rs`.

  **The EXPECT's `1060 + 3` is arithmetic the push outran.** The desktop crate
  was 594 at G125, and this slice's four tests -- three in `install.rs`, one in
  `wire.rs` -- make it 598, so the workspace total is 1073 rather than 1063. The
  prediction named the count it expected; the run named the count it made, and
  the run is the number.

- [x] G127: the two Forge-shaped loaders resolve from their installer's own
  translated profile -- the resolve asks the jar the launch has already run,
  rather than Prism's mirror, and neither uid is dialled at
  meta.prismlauncher.org
  CHECK: cargo test -p palantir-desktop --locked
         cargo clippy -p palantir-desktop --all-targets --locked -- -D clippy::correctness
         python tools/progress.py --check
         python tools/dashboard.py --check
  EXPECT: `published_loader` names `net.minecraftforge` and `net.neoforged`, and
          `PublisherMeta::version_file` answers both out of `InstallerMeta::profile`
          -- the same call G99 and G100 measure against a real Forge and a real
          NeoForge -- while Fabric's and Quilt's profiles still come from
          `LoaderMeta`
          `a_forge_instance_resolves_its_loader_from_the_installers_own_profile`
          fetches the installer at its maven URL with the digest its `.sha1`
          states and returns `net.minecraftforge.bootstrap.ForgeBootstrap`, a
          class the mirror's ForgeWrapper copy does not name, with the mirror's
          base a URL the scripted service has nothing at -- so a question asked
          of it would come back as a problem rather than be hidden by a hit
          an instance whose pack profile names no game version still leaves the
          loader to the mirror, and the mappings components an imported instance
          may list stay the mirror's
          the desktop crate's 604 tests become 605, and clippy exits 0 with no
          new warning in `meta.rs`
          both document tools exit 0, the ledger now carrying 128 gates
  EVIDENCE: G107 read what the mirror serves for these two uids and named why it
            is not this launcher's, G126 landed the install that has to run
            before a resolve can use the installer's profile, and G99/G100 are
            the measurements of that profile against real builds. This is the
            routing flip between them, in the order G107 named.

  **Why the mirror was never the answer for these two.** G107 read Prism's
  `net.minecraftforge` and `net.neoforged` profiles and found they are not the
  loader's file at all: the main class is
  `io.github.zekerzhayard.forgewrapper.installer.Main`, and a second key,
  `mavenFiles`, holds the loader's own installer plus neoform, installertools and
  binarypatcher. That is a *wrapper* which runs an installer at first launch, and
  `install::plan` -- the launcher's own install -- walks `libraries`,
  `native_libraries` and `main_jar` only, so the wrapper's `maven_files` are
  parsed and never fetched: resolving that profile launches a launcher sitting
  next to a loader nothing put there. The loader's own answer is the
  `version.json` inside its installer, which G99 measured against the mirror
  library for library, and which G100 proved is a profile an install can finish.

  **The order was not a preference, and this half is why.** A launch resolves
  through `PublisherMeta`, so a flip that came before G126's install would resolve
  the installer's profile against an instance whose client jar the processors have
  not patched yet: the libraries the profile names are not on disk and the main
  class launches a client that was never built. G126 runs the install in
  `prepare_launch` before the resolve, which is what makes the installer's own
  profile the honest source for a launch rather than an optimistic one. The two
  halves are the pair G107 named, landed in the order it named.

  **What it changes in the code is one match arm.** `published_loader` names the
  two uids; `PublisherMeta` gains the `InstallerMeta` and `ContentStore` the wire
  already holds (`Wire::installers`, `Wire::content`), so the installer G126
  fetched is a cache hit here rather than a second download; and
  `version_file`'s `Publisher` arm splits -- Fabric and Quilt still come from
  `LoaderMeta`, the Forge-shaped two from `InstallerMeta::profile`. The content
  store is the same `content/` a launch's own downloads use, which is what makes
  the two halves one install rather than two copies of one.

  **What the offline test proves, and what it does not.** It proves the routing,
  the fetch at the installer's maven URL under the digest its own `.sha1` states,
  and the translated launch class that comes back -- with the mirror's base a URL
  the scripted service has nothing at, so a mirror answer would fail rather than
  pass quietly. It cannot prove that a real Forge build resolves end to end,
  because that is G99's and G100's work and they live in `palantir-net`'s
  `#[ignore]`d live suite: `PublisherMeta` is in the desktop crate, so no live
  test reaches this arm. The live receipt is those two tests re-run on this
  commit by the `Live services` job, over the same `InstallerMeta::profile` the
  arm calls and a real Forge 1.21.1/52.1.0 and NeoForge 1.21.1/21.1.172.

  The runner's own numbers are recorded below, in the paragraph this entry gains
  once its push has one to read.

- [x] G128: an instance's installation is a form too -- the platform, the game
  version and the loader's build, read from its own `mmc-pack.json` and written
  back to the same profile a launch resolves
  CHECK: cargo test -p palantir-desktop --locked
         cargo clippy -p palantir-desktop --all-targets --locked -- -D clippy::correctness
         python tools/progress.py
         python tools/dashboard.py --check
  EXPECT: the settings modal's second tab opens on what the creation flow wrote:
          the platform, the game version and the loader's build, all three read
          from the instance's own `mmc-pack.json`
          the game-version list is filtered by its search and the snapshot toggle,
          and the loader-build list is fetched for the `(platform, game version)`
          pair in force -- a change to either clears the build chosen for the old
          pair, because a build number belongs to one loader at one game version
          a save writes `net.minecraft`'s version and the platform's own component,
          and *takes every other loader out of the profile*: two loader components
          is a profile nothing can resolve, so a switch is a write and a removal.
          A switch to vanilla removes the loader and leaves the game version
          a profile carrying a loader this launcher does not model (Prism's
          LiteLoader) is a refusal rather than a deletion, and a non-vanilla
          platform with no build is refused by the store
          600 passed; 0 failed; 0 ignored, and clippy exit 0 with no new warning in
          the files this slice touched
          the plan's stage-3 installation item narrows to the modpack half -- the
          panel whose first requirement is bookkeeping this launcher does not keep
          yet -- stage 3 reads 32 met / 2 open (94%), and both document tools exit
          0 at 127 gates
  EVIDENCE: the transcripts below, and the three drawn differences from the
  reference, each recorded where it is drawn rather than assumed.

```
$ cargo test -p palantir-desktop --locked
running 600 tests
test result: ok. 600 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
running 4 tests
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

$ cargo clippy -p palantir-desktop --all-targets --locked -- -D clippy::correctness
warning: `palantir-desktop` (bin "PalantirMC") generated 3 warnings
exit 0; the bin's 3 and the test bin's 21 (1 duplicate) are the counts the
settings modal's own commit read, so this slice adds none -- and the only
diagnostic naming a file it touched is the pre-existing `unnecessary_sort_by` at
catalog.rs:142, a `#[cfg(test)]` helper the slice did not change (it is in the 21,
and it is in the logs from G90 onward)

$ python tools/progress.py
stage 3  Pages, in the reference's order: instance pages f...    32     2    94%

$ python tools/dashboard.py --check
CONFIRMED: the page carries all 127 gates, 6 stage cards and every subject as written
```

  **What this is, and where the reference keeps it.** The reference's
  `InstanceSettingsModal` has an `installation` tab whose component is
  `components/settings-modal/installation-settings.vue`: three choices -- the
  platform, the game version and the loader's build -- over a `layout.vue` that
  draws them as rows, each with a select, and a Save whose `afterSave` re-runs the
  install. The values behind those three rows are the instance's own
  `mmc-pack.json`: `net.minecraft`'s version, and the loader component's uid and
  version. This slice is that tab: `crate::instance_settings` grew a tab strip and
  the second form, `store::InstanceInstallation` is the value it reads and writes,
  and the shell routes the tab's two service reads the way it routes the creation
  dialog's.

  **The write, and the two rules it keeps.** A save writes the game version onto
  `net.minecraft` and the build onto the platform's component, and it *removes*
  every other loader the profile carries -- `ModLoader::conflicting_uids` is the
  same rule stated by the model, and a profile with two loaders is one nothing can
  resolve. Our own writes mark a loader `important` and `PackProfile::remove`
  refuses an important component (Prism's rule: the reader asked for it), so the
  removal clears that flag first, which is this tab's way of saying the reader is
  asking again. And a component this launcher does not model -- Prism's
  LiteLoader, which `LoaderKind` has no uid for -- is a *refusal* rather than a
  deletion: a save that silently dropped it would be this tab editing a file it
  cannot read. What a changed version or platform costs is the next launch's
  download, because the launch resolves the components it finds; the reference
  re-runs its install from the modal instead, and the sentence under this tab's
  Save says which one happens.

  **The three drawn differences, each with its reason.** The reference's choices
  are comboboxes filtered by the loader's own manifest (`resolveGameVersions`
  keeps only the versions that manifest lists); this kit has no combobox of that
  shape, so each choice is the chips-and-search rows the creation dialog already
  draws, and the *filter* is the service's rather than a second document's: the
  store asks the loader for the builds of the chosen game version
  (`LoaderMeta::builds`) and an empty answer draws the reference's own *no
  versions available* sentence, so a version the loader never published for is
  answered rather than hidden. A platform or game-version change clears the build
  chosen for the old pair, because a Fabric build number under a Quilt heading is
  not a value any profile can resolve. And the tab strip is this modal's own: the
  reference's tabs are General, Installation, Sync overrides and Sharing, with the
  Java settings drawn inside the sync-overrides tab -- which this launcher does not
  have (G118), so those settings took the tab beside Installation, labelled with
  this module's word where the reference's own there are none.

  **What is not this half.** The reference's installation tab also carries the
  panel for an instance installed from a Modrinth pack -- the pack and version,
  *Repair*, *Reinstall*, *Swap*, *Unlink* -- which is not a write onto the
  instance file: *Repair* re-runs the install and *Unlink* forgets the link. Its
  first requirement is bookkeeping this launcher does not keep yet (an instance
  does not remember the project and version `crate::install` wrote it from), and it
  stays the plan's open item, named in the module doc and in the plan.

  **The id moved again, and this time it is this entry that moved.** The work
  order named this slice G127, and it was written and verified as that number
  before its push -- but G126's own commit, which landed first, reserves G127 for
  the Forge-shaped loaders' routing flip and says so in its entry. The mirror's
  tip owns the number, so this installation tab is G128 and G127 is left for that
  flip. Nothing about the slice changed with the number: the plan's
  `GATE_OWNERS` entry, the stage-3 open list and this ledger line were the three
  places it appeared.

  **The runner agrees, and its numbers are the rebased tree's.** Run
  `36690942443` (`b2d1582`, the commit that also carries G126): `Test workspace`
  and `Lint` green in 2m25s and 1m31s, 1079 passed / 0 failed and 18 ignored
  (177 + 8 + 604 + 4 + 31 + 255). The desktop crate's row reads 604 rather than
  this slice's own 600 because G126's installers ride in the same commit and add
  four; the local transcript above is the slice alone. `Live services` and
  `Build exe` are skipped, which is the pull-request path's own rule rather than
  a failure -- the four-job path is what a push to the branch gets, and the
  five-job run is asked for separately. The lint job marks one red annotation
  again, the advisory `cargo fmt --check` step it runs `continue-on-error`; the
  job's own conclusion is success, which is why a red line and a green job appear
  together.

- [x] G129: an instance installed from a Modrinth pack remembers which project and
  version it came from, and the settings modal draws the reference's own card over
  it -- the pack named from the service, and *Unlink modpack*
  CHECK: cargo test -p palantir-desktop --locked
         cargo clippy -p palantir-desktop --all-targets --locked -- -D clippy::correctness
         python tools/progress.py
         python tools/dashboard.py --check
  EXPECT: installing a pack writes its link beside the profile the install wrote --
          `modrinth-link.json`, carrying the reference's own `type`, project id and
          version id -- and the installation tab draws it: the *Installed modpack*
          card, named from the project document and the version list rather than
          from the file, so a project renamed shows under its new name
          *Unlink modpack* takes the link and nothing else: the instance, its
          `mmc-pack.json` and every file the pack install put in it stay
          an instance nobody linked asks no service anything, and a link file that
          cannot be read is a sentence in the tab rather than a modal that draws
          nothing but a complaint
          610 passed; 0 failed; 0 ignored, and clippy exit 0 with no new warning in
          the files this slice touched
          the plan's installation item narrows from the panel to the three actions
          that re-run an install -- stage 3 reads 33 met / 2 open (94%) -- and both
          document tools exit 0 at 128 gates
  EVIDENCE: the transcripts below, and the one drawn difference (no icon on the
  card) recorded where it is drawn rather than assumed.

```
$ cargo test -p palantir-desktop --locked
running 610 tests
test result: ok. 610 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
running 4 tests
test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out

$ cargo clippy -p palantir-desktop --all-targets --locked -- -D clippy::correctness
exit 0; the bin's 3 and the test bin's 21 (1 duplicate), the counts G125 and G128
both read, so this slice adds none

$ python tools/progress.py
stage 3  Pages, in the reference's order: instance pages f...    33     2    94%

$ python tools/dashboard.py --check
CONFIRMED: the page carries all 128 gates, 6 stage cards and every subject as written
```

  **What this is, and where the reference keeps it.** The reference stores the
  link on the instance (`InstanceLink`, `{ type: 'modrinth_modpack', project_id,
  version_id }`, inside its own `profile.json`) and names it from the service
  (`get_linked_modpack_info`, which is what its card draws). This launcher's
  instances are Prism-shaped and Prism has no such field, so the link is a file of
  its own beside `mmc-pack.json` (`crate::store::LINK_FILE`), with the reference's
  field names inside it -- deliberately not a key in `mmc-pack.json`, which is a
  component list a launch resolves, and not a rewrite of a file another launcher
  owns. `Store::install_pack` writes it where the project and version are still in
  hand, before the answer, so an instance that appears in the library is one whose
  own modal can say what it came from; a link that would not write is not an
  install that failed, and the line the page draws carries the reason instead.

  **The card, and the one action that is a file.** The tab draws the reference's
  own `Installed modpack` card, named by `Store::linked_modpack` -- three requests
  (the document, the team, the version list), all cached by the engine, with the
  author and the version number deliberately allowed to be missing: an author is a
  caption on a team read that can fail, and a version its author has deleted is a
  version no card can name, which is the reference's own `version?.version_number`.
  *Unlink modpack* is the first of the panel's four actions to land, and it is the
  one that is a file write rather than an install: it removes the link and nothing
  else, which is what the reference's own sentence says it does. The form stays up
  when it is pressed, and the link is read *again* rather than assumed gone -- a
  file this launcher could not remove is a sentence in the form instead of a card
  that vanishes while the file it was drawn from is still there.

  **A race this slice's own tests found, and the shape to avoid.** The first draft
  of the two link tests was built on `Store::default()`, whose `instances_dir` is
  the *empty* path -- so `instance_dir("atm10")` was the relative `atm10`, one
  folder shared by every test in that process, and the two tests wrote one
  `modrinth-link.json` between them. It passed on the first run and failed on the
  second with each test reading the other's link, which is what a race looks like
  when the fixture is the bug. Both now run over a real instances directory
  (`store_without_instances`), and the trap is written down here because
  `Store::default()` looks like the cheap way to build a read-only fixture and is
  not one for a test that writes: a default store's instance folder is wherever
  the test binary was run from.

  **What is still owed, and the one drawn difference.** *Change version* (the
  reference's *Swap*), *Re-install modpack* and *Repair instance* are not built:
  each of the three re-runs an install rather than editing a file, which is
  `crate::install`'s job rather than this modal's, and the plan's own item says so.
  The card is also drawn without the project's picture, which the reference puts
  beside its title on the same row: an icon is a fetch and a decode, this kit has
  that path in exactly one place (`crate::pages::user`'s avatar), and what the card
  is asked is which pack this instance came from -- which the words answer.

  **The runner agrees, and its numbers are the rebased tree's.** Run
  `36694325430` (`f474588`, the commit that also carries G127's routing flip):
  `Test workspace` and `Lint` green in 3m00s and 1m29s, 1086 passed / 0 failed and
  18 ignored (177 + 8 + 611 + 4 + 31 + 255). The desktop crate's row reads 611
  where the local transcript above reads 610, because G127's flip rides in the same
  commit and its tests are the difference; the local run is this slice alone, over
  the tree before that rebase. `Live services` and `Build exe` are skipped, which
  is the pull-request path's own rule rather than a failure, and the lint job marks
  its usual one red annotation -- the advisory `cargo fmt --check` step it runs
  `continue-on-error`, beside a job whose own conclusion is success.

## What these gates cannot say

- **No gate compares glyph bitmaps between the clients.** Their ClearType colour
  fringing makes the same word two different pictures, so every text assertion
  here is about ink rows, ink colour and position rather than about pixels.
- **No gate says the copy is used.** `theme_tokens.rs` is compiled for tests
  only (`#[cfg(test)]` in `main.rs`): it is the receipt for the 189 tokens, not
  the thing the shell paints from. 163 of them are held by nothing yet, and a
  gate cannot see a page that has not been ported.
- **No capture of the loading page.** The splash's numbers are asserted as
  functions and its tokens as composited colours; what it *looks like* on screen
  has been reasoned from the reference's own stylesheet rather than photographed
  on both clients side by side, which is the one thing `G8` above asks for and
  this page does not yet have.
- **No gate measures the engine's ceiling under load.** `Limit` is tested
  directly, including the peak it reached under contention, but nothing asserts
  that eleven simultaneous downloads draw eight permits from `HttpPool`: that
  needs a server to be slow on purpose, and there is no such server here. The
  claim currently rests on the code path -- one `acquire` per request, held for
  the body's life -- and on the two live tests that do make real requests.
- **No gate says a service tells the truth about a validator.** The cache is
  tested against a service that republishes (new body, new `ETag`) and one that
  sends no `ETag` at all, but a service that changes a body and keeps the same
  `ETag` would be believed, and no test can produce that from outside the
  engine: the only defence is the digest inside the document, which is the
  caller's to check.
- **One page is switched onto the engine.** Discover's search is the only request
  that goes through the seam; the right panel, the settings modal and every
  instance-facing list still answer from disk or from the copy, so this document
  says nothing yet about them being served by the code above. Stage 5 does not
  cover it either: what is left is the panel's other sections and every page that
  is not Discover, which is what stage 3's own open list names.
