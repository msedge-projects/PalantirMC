# Gates: PalantirMC's shell matches the reference client

OWNS: crates/prism-desktop/src/**, tools/panel_gate.py, NEXT_STEPS.md

Scope: the shell's chrome, surfaces, shape and type match the Modrinth App as
measured off its own running window rather than estimated from screenshots, and
the measurement survives as a command.

The oracle is `tools/panel_gate.py`. Every expectation below is a record of what
the reference client's own pixels measured, and each assertion is written so that
the build it replaced fails it -- which is the only thing that makes a passing
run mean anything. `G7` is the gate that proves that claim.

Captures are `PrintWindow` grabs from `tools/winshot.py`, taken off-screen and
without activating the window. `.scratch/pal-final.png` is the capture under
test; `.scratch/mr-home.png` and `.scratch/pal-new.png` are the control's two
inputs and are deliberately *not* committed -- the first is a screenshot of
another product, and the second is a superseded build. G7 is therefore
environment-dependent: it is runnable here and stands as a recorded manual
result elsewhere. The reference numbers G1--G6 assert against are baked into
the checker, so those gates need nothing but the capture under test.

- [ ] G1: the right panel's gutter carries the brand tint, where the build it
      replaced painted the neutral raised grey (`#27292e`, whose green sits five
      levels *below* its blue)
  CHECK: python tools/panel_gate.py .scratch/pal-final.png --only gutter
  EXPECT: panel gate passed [gutter]
  EVIDENCE: pending

- [ ] G2: the panel's wash ramps downward across the window, the way the
      reference's does, rather than being flat
  CHECK: python tools/panel_gate.py .scratch/pal-final.png --only ramp
  EXPECT: panel gate passed [ramp]
  EVIDENCE: pending

- [ ] G3: the panel's wash lands on the reference's own two measured stops,
      `#182524` at the top and `#131a1a` at the bottom, within 5 per channel
  CHECK: python tools/panel_gate.py .scratch/pal-final.png --only matches
  EXPECT: panel gate passed [matches]
  EVIDENCE: pending

- [ ] G4: the page pane still matches the reference, i.e. the panel's wash did
      not leak into it
  CHECK: python tools/panel_gate.py .scratch/pal-final.png --only page
  EXPECT: panel gate passed [page]
  EVIDENCE: pending

- [ ] G5: the page pane's top-left corner is cut away so the chrome shows
      through -- `.app-contents`'s `--radius-xl` -- rather than the pane being a
      square rectangle
  CHECK: python tools/panel_gate.py .scratch/pal-final.png --only corner
  EXPECT: panel gate passed [corner]
  EVIDENCE: pending

- [ ] G6: a card inside the panel is brand-tinted like the reference's, not the
      neutral raised grey
  CHECK: python tools/panel_gate.py .scratch/pal-final.png --only card
  EXPECT: panel gate passed [card]
  EVIDENCE: pending

- [ ] G7: the oracle discriminates -- it passes on the reference's own capture
      and fails on the build it replaced, so a pass is a real verdict and not a
      checker that cannot fail
  CHECK: python tools/panel_gate.py .scratch/mr-home.png .scratch/pal-new.png --control
  EXPECT: oracle discriminates
  EVIDENCE: pending

- [ ] G8: the two windows agree as a picture, judged by looking at them side by
      side rather than by any number -- the acceptance the user actually asked
      for, and the one no assertion above can stand in for
  EVIDENCE: pending
