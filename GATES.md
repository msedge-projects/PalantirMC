# Gates: PalantirMC's shell matches the reference client

OWNS: crates/palantir-desktop/src/**, tools/panel_gate.py, tools/page_gate.py,
tools/appshot.py, NEXT_STEPS.md

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
with the three captures that show the oracle discriminating.

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
