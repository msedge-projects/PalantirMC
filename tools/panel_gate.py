"""Assert that the shell's chrome matches the reference client, off one capture.

The reference numbers below are not targets chosen by hand: they were measured
out of a print-window capture of the Modrinth App itself and recorded in
`NEXT_STEPS.md` §11, together with how each one was sampled. This script is the
oracle for those measurements, so that "it matches now" is a command's verdict
rather than a claim.

    python tools/panel_gate.py .scratch/pal-new.png

Exits 0 and prints `panel gate passed` only when every assertion holds.

Every assertion is stated so that the build it replaced fails it, which is the
only thing that makes a passing run mean anything. Both controls are recorded:

    * `.scratch/mr-home.png`   -- the reference itself; must pass (5 of 7 gates
      cannot be judged here because the reference's own screenshot shows a card
      in the gutter that ours has not, see the ledger)
    * `.scratch/pal-new.png`   -- the build before this one; must fail the tint,
      the ramp and the pane-corner gates

Sampling is structural -- edges are found, not assumed -- so a window of another
size, or on another DPI scale, still measures the same thing.
"""
from __future__ import annotations

import sys
from pathlib import Path

try:
    from PIL import Image
except ImportError:  # pragma: no cover - environment guard, not logic
    print("Pillow is required: python -m pip install pillow")
    raise SystemExit(2)

# ---- The reference client's own pixels, measured from its capture ----------
REF = {
    "panel_top": (0x18, 0x25, 0x24),  # empty gutter, just under the title bar
    "panel_bottom": (0x13, 0x1A, 0x1A),  # the same gutter near the bottom
    "panel_card": (0x2A, 0x36, 0x33),  # a card inside the panel
    "panel_row": (0x3A, 0x43, 0x41),  # a row inside such a card
    "page": (0x16, 0x18, 0x1C),  # the page pane
    "chrome": (0x27, 0x29, 0x2E),  # the raised chrome that frames it
}

TOL = 6


def lum(c):
    return 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2]


def near(c, ref, tol=TOL):
    return all(abs(a - b) <= tol for a, b in zip(c, ref))


def diff(a, b):
    return max(abs(x - y) for x, y in zip(a, b))


def crop_window_frame(im):
    """Drop a native window border, if the capture carries one.

    `PrintWindow` on a DPI-virtualised process returns the window plus its DWM
    frame, which the reference capture has and ours does not. Detected rather
    than assumed, because a stray black border would otherwise become the
    panel's background and silently move every sample.
    """
    w, h = im.size

    def is_black(c):
        return all(v <= 8 for v in c)

    def column_black(x):
        return all(is_black(im.getpixel((x, y))) for y in range(0, h, 7))

    def row_black(y):
        return all(is_black(im.getpixel((x, y))) for x in range(0, w, 7))

    if not column_black(0):
        return im, 0

    left = 0
    while left < 12 and column_black(left):
        left += 1
    top = 0
    while top < 12 and row_black(top):
        top += 1
    right = w - 1
    while right > w - 12 and column_black(right):
        right -= 1
    bottom = h - 1
    while bottom > h - 12 and row_black(bottom):
        bottom -= 1
    return im.crop((left, top, right + 1, bottom + 1)), left


def vertical_edge(im, lo, hi):
    """The x in [lo, hi) with the strongest vertical discontinuity.

    A surface boundary runs the full height of the window, so counting how many
    sampled rows change across a column finds it even when a card or a label
    covers part of the column -- which is exactly what fixed coordinates got
    wrong.
    """
    w, h = im.size
    best, best_score = None, 0
    ys = range(2, h - 2, 3)
    for x in range(max(lo, 1), min(hi, w)):
        score = sum(
            1 for y in ys if diff(im.getpixel((x, y)), im.getpixel((x - 1, y))) > 6
        )
        if score > best_score:
            best, best_score = x, score
    return best, best_score


def horizontal_edge(im, lo, hi):
    """The y in [lo, hi) with the strongest horizontal discontinuity."""
    w, h = im.size
    best, best_score = None, 0
    xs = range(2, w - 2, 3)
    for y in range(max(lo, 1), min(hi, h)):
        score = sum(
            1 for x in xs if diff(im.getpixel((x, y)), im.getpixel((x, y - 1))) > 6
        )
        if score > best_score:
            best, best_score = y, score
    return best, best_score


class Gate:
    """Collects assertions, and can be narrowed to one named assertion.

    Narrowing is what lets each gate in the ledger own a command and a distinct
    `EXPECT` line rather than six gates sharing one run, so a failure can be
    attributed to the gate it belongs to.
    """

    def __init__(self, only: str | None = None) -> None:
        self.failures: list[str] = []
        self.only = only
        self.ran: list[str] = []

    def selected(self, label: str) -> bool:
        return self.only is None or self.only in label

    def check(self, ok: bool, label: str, detail: str) -> None:
        if not self.selected(label):
            return
        self.ran.append(label)
        print(f"  [{'ok' if ok else 'FAIL'}] {label}: {detail}")
        if not ok:
            self.failures.append(label)


def control(reference: str, candidate: str) -> int:
    """Prove the oracle discriminates before trusting its verdict.

    A gate that cannot fail proves nothing, so this runs the same checker over
    the reference (which must pass) and over the build it replaced (which must
    fail). A checker that passes both, or fails both, is broken.
    """
    import subprocess

    def verdict(p):
        r = subprocess.run(
            [sys.executable, __file__, p], capture_output=True, text=True
        )
        return r.returncode, r.stdout.strip().splitlines()[-1] if r.stdout.strip() else ""

    ref_code, ref_last = verdict(reference)
    cand_code, cand_last = verdict(candidate)
    print(f"reference {reference}: exit {ref_code} -- {ref_last}")
    print(f"candidate {candidate}: exit {cand_code} -- {cand_last}")
    if ref_code == 0 and cand_code != 0:
        print("oracle discriminates")
        return 0
    print("oracle does NOT discriminate")
    return 1


def main() -> int:
    argv = sys.argv[1:]
    only = None
    if "--only" in argv:
        i = argv.index("--only")
        only = argv[i + 1]
        del argv[i : i + 2]
    if "--control" in argv:
        argv.remove("--control")
        if len(argv) != 2:
            print("--control needs both captures: <reference.png> <candidate.png>")
            return 2
        return control(argv[0], argv[1])
    if len(argv) != 1:
        print(__doc__)
        return 2

    path = Path(argv[0])
    if not path.exists():
        print(f"capture not found: {path}")
        return 2

    im, frame = crop_window_frame(Image.open(path).convert("RGB"))
    w, h = im.size
    print(f"capture {path.name}: {w}x{h}" + (f" (cropped a {frame}px frame)" if frame else ""))
    g = Gate(only)

    def px(x, y):
        return im.getpixel((max(0, min(x, w - 1)), max(0, min(y, h - 1))))

    # The panel is a full-height column on the right; its left edge is the last
    # strong vertical boundary on that side.
    # The last 4 columns are excluded: a capture that carried a window frame
    # leaves a half-black transition column at the edge after cropping, and it
    # out-scores the real boundary.
    panel_left, panel_score = vertical_edge(im, int(w * 0.62), w - 4)
    # The rail's right edge is the first strong one on the left.
    pane_left, pane_score = vertical_edge(im, int(w * 0.02), int(w * 0.2))
    bar_bottom, bar_score = horizontal_edge(im, int(h * 0.04), int(h * 0.16))
    print(
        f"edges: panel x={panel_left} (score {panel_score}/{h // 3}), "
        f"rail x={pane_left} (score {pane_score}/{h // 3}), bar y={bar_bottom} (score {bar_score}/{w // 3})"
    )

    g.check(
        all(s > (h // 3) * 0.5 for s in (panel_score, pane_score))
        and bar_score > (w // 3) * 0.5,
        "the three chrome boundaries are found",
        f"panel {panel_score}, rail {pane_score}, bar {bar_score} rows/cols changed",
    )
    if panel_left is None or pane_left is None or bar_bottom is None:
        print("\npanel gate FAILED: boundaries not found")
        return 1

    # The panel's background, taken as the median of its right-hand gutter: the
    # 16px of padding no card reaches into.
    gutter_x = w - 4
    column = [px(gutter_x, y) for y in range(bar_bottom + 4, h - 4, 4)]
    panel_bg = tuple(sorted(c[i] for c in column)[len(column) // 2] for i in range(3))
    print(f"panel background (median of its gutter) #%02x%02x%02x" % panel_bg)

    g.check(
        150 < (w - panel_left) < 400,
        "panel is a panel's width",
        f"{w - panel_left}px of {w} (reference: 237 of 1088, i.e. 300 logical)",
    )

    def gutter(y0, direction):
        """The first panel-background pixel scanning away from y0."""
        for i in range(0, 14):
            y = y0 + i * direction
            c = px(gutter_x, y)
            if near(c, panel_bg, 4):
                return c
        return px(gutter_x, y0)

    top = gutter(bar_bottom + 20, +1)
    bottom = gutter(h - 24, -1)
    print("gutter top #%02x%02x%02x  bottom #%02x%02x%02x" % (top + bottom))

    # G1: tinted with the brand green rather than neutral grey. The reference's
    # own gutter measures +2 (`#172321`); the build this replaced painted
    # `surface-3` (39, 41, 46), whose green is *below* its blue at -5. The
    # threshold is the reference's measurement, not a round number.
    g.check(
        top[1] - top[2] >= 2,
        "panel gutter is brand-tinted",
        f"green-blue = {top[1] - top[2]:+d} (want >= +2; reference +2, previous build -5)",
    )

    # G2: the wash ramps downward, as the reference's does.
    g.check(
        top[1] > bottom[1],
        "panel wash ramps downward",
        f"green {top[1]} at the top -> {bottom[1]} at the bottom",
    )

    # G3/G4: and lands on the reference's own values at both ends.
    g.check(near(top, REF["panel_top"], 5), "panel top matches reference", "#%02x%02x%02x" % top)
    g.check(
        near(bottom, REF["panel_bottom"], 5),
        "panel bottom matches reference",
        "#%02x%02x%02x" % bottom,
    )

    # G5: the page pane still matches, i.e. the wash did not leak into it.
    # Sampled 5px inside the pane's own left padding and at mid-height: both
    # clients keep that strip clear for the whole height, whereas the bottom of
    # the pane is a status bar here and page background there -- sampling low
    # would compare two different widgets.
    pane_probe = (pane_left + 5, (bar_bottom + h) // 2)
    pane = px(*pane_probe)
    g.check(
        near(pane, REF["page"], 4),
        "page pane matches reference",
        "#%02x%02x%02x at x={}".format(pane_probe[0]) % pane,
    )

    # G6: the pane's top-left corner is cut, so the chrome shows through it.
    # A square pane paints its own colour there; a rounded one reveals the
    # chrome underneath, which is measurably lighter. A 20px radius puts the arc
    # ~6px in from each edge, so 3px in is outside the pane.
    pane_top = None
    for y in range(bar_bottom, bar_bottom + 20):
        if near(px(pane_left + 60, y), REF["page"], 4):
            pane_top = y
            break
    if pane_top is None:
        g.check(False, "pane corner is cut, showing chrome", "page did not start below the bar")
    else:
        corner = px(pane_left + 3, pane_top + 3)
        inside = px(pane_left + 60, pane_top + 8)
        print("pane corner #%02x%02x%02x  (pane interior #%02x%02x%02x)" % (corner + inside))
        g.check(
            lum(corner) - lum(inside) >= 8,
            "pane corner is cut, showing chrome",
            f"corner luminance {lum(corner):.1f} vs pane {lum(inside):.1f} (want >= +8)",
        )

    # G7: a card in the panel is brand-tinted too, not the neutral raised grey.
    counts: dict[tuple[int, int, int], int] = {}
    for y in range(bar_bottom + 8, min(bar_bottom + 320, h), 2):
        for x in range(panel_left + 6, w - 6, 2):
            c = px(x, y)
            if near(c, panel_bg, 5) or near(c, REF["chrome"], 5):
                continue
            counts[c] = counts.get(c, 0) + 1
    if counts:
        card = max(counts, key=counts.get)
        print(
            "panel's most common non-background colour #%02x%02x%02x (%d px)"
            % (card + (counts[card],))
        )
        # The dominant colour there is whichever of the two panel surfaces covers
        # more of the region -- a card or a row inside one -- so either token is
        # a pass, and the reference likewise hits its own `panel_row`.
        matches = near(card, REF["panel_card"], 16) or near(card, REF["panel_row"], 16)
        g.check(
            card[1] > card[2] and matches,
            "panel card is brand-tinted like the reference",
            "#%02x%02x%02x against card #%02x%02x%02x / row #%02x%02x%02x"
            % (card + REF["panel_card"] + REF["panel_row"]),
        )
    else:
        g.check(False, "panel card is brand-tinted like the reference", "no card found")

    print()
    if not g.ran:
        print(f"no assertion matched --only {only!r}")
        return 2
    scope = f" [{only}]" if only else ""
    if g.failures:
        print(f"panel gate FAILED{scope} ({len(g.failures)}): " + ", ".join(g.failures))
        return 1
    print(f"panel gate passed{scope}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
