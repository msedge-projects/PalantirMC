# Gates: PalantirMC's shell matches the reference client

OWNS: crates/palantir-desktop/src/**, tools/panel_gate.py, tools/page_gate.py,
tools/appshot.py, tools/shellcmp.py, NEXT_STEPS.md

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
