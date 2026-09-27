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
`36241262447`, `36242305444`, `36254170068`, and every push adds one — each died
in three to six seconds with zero steps and
the same annotation: `recent account payments have failed or your spending limit
needs to be increased`. So no job ran, in either workflow, and there is no
`test result` line from a runner to quote for any of them. The ids are written
out because a blocked run is a fact about the account and not a verdict on the
tree, and the only way a later reader can tell the two apart is if both are named
the same way. The commands `ci.yml` runs were run here
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
