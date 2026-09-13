# Gates: PalantirMC's shell matches the reference client

OWNS: crates/prism-desktop/src/**, tools/panel_gate.py, NEXT_STEPS.md

Scope: the shell's chrome, surfaces, shape and type match the Modrinth App as
measured off its own running window rather than estimated from screenshots, and
the measurement survives as a command.

The oracle is `tools/panel_gate.py`. Every expectation below is a record of what
the reference client's own pixels measured, and each assertion is written so that
the build it replaced fails it -- which is the only thing that makes a passing
run mean anything. G7 is the gate that proves that claim rather than assuming it.

Captures are `PrintWindow` grabs from `tools/winshot.py`, taken off-screen and
without activating the window. `.scratch/pal-final.png` is the capture under
test; `.scratch/mr-home.png` and `.scratch/pal-new.png` are the control's two
inputs and are deliberately *not* committed -- the first is a screenshot of
another product, and the second is a superseded build. G7 is therefore
environment-dependent: it is runnable here and stands as a recorded manual
result elsewhere. The reference numbers G1--G6 assert against are baked into
the checker, so those gates need nothing but the capture under test.

Result: **7 met, 0 unmet, 0 abandoned** for the build at `84d5f4a`, captured into
`.scratch/pal-final.png` (1257x707). G8 is met by inspection.

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

- [x] G8: the two windows agree as a picture, judged by looking at them side by
      side rather than by any number -- the acceptance the user actually asked
      for, and the one no assertion above can stand in for
  EVIDENCE: inspected `.scratch/pal-final.png` against `.scratch/mr-home.png`.
      The page reads as a rounded panel inset in the chrome, the panel is a dark
      green-tinted column, and the checklist shows a hollow ring for the
      outstanding step and green discs with checks for the two complete ones --
      the reference's own marker vocabulary. Differences that remain and are not
      claimed by any gate above are recorded in `NEXT_STEPS.md` §12.
