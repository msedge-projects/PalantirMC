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
    # Not from the panel: these two are the *brand's* own fills, measured off the
    # call-to-action button ("+ Create an instance") and off the plate behind
    # whichever rail entry is active. Both are flat -- 75% and 86% of their own
    # regions -- which is what makes a modal colour the right instrument for
    # them. See tools/refsample.py and REFERENCE.md for the samples.
    "accent": (0x00, 0xDA, 0x75),  # the brand green as the app paints it
    "plate": (0x1D, 0x55, 0x40),  # accent at 25% over the chrome, as a solid
}

TOL = 6

# Both clients keep their outermost pixels for something that is not layout: this
# shell draws a 6px band of resize grips on every edge, the reference carries a
# window frame. Either one is a far stronger vertical edge than the panel's own
# boundary -- which is a brand wash against a page colour, only a few levels
# apart -- so the strongest-edge search latched onto the band and reported the
# panel as 6px wide. Dropping a uniform inset first removes it from both.
INSET = 4


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
    w0, h0 = im.size
    im = im.crop((INSET, INSET, w0 - INSET, h0 - INSET))
    w, h = im.size
    print(
        f"capture {path.name}: {w0}x{h0}"
        + (f" (cropped a {frame}px frame)" if frame else "")
        + f" -> {w}x{h} after a {INSET}px inset"
    )
    g = Gate(only)

    def px(x, y):
        return im.getpixel((max(0, min(x, w - 1)), max(0, min(y, h - 1))))

    # The panel is a full-height column on the right; its left edge is the last
    # strong vertical boundary on that side.
    panel_left, panel_score = vertical_edge(im, int(w * 0.62), w - 2)
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

    # The panel's background column, located rather than assumed. A fixed
    # offset does not survive: the panel's padding is 16 logical px, which is 13
    # physical px in the reference's capture and 16 in ours, and this shell also
    # draws a scrollbar in the same band where the reference overlays one. The
    # gutter is the *darkest* column in the panel's right margin, because every
    # neighbour it could be confused with -- a card, a scrollbar track -- is
    # lighter than the panel it sits on.
    def column_median(x):
        vals = [px(x, y) for y in range(bar_bottom + 4, h - 4, 4)]
        return tuple(sorted(v[i] for v in vals)[len(vals) // 2] for i in range(3))

    def strip(x, y0):
        vals = [px(x, y) for y in range(y0 - 6, y0 + 7)]
        return tuple(sorted(v[i] for v in vals)[len(vals) // 2] for i in range(3))

    # Find a clear gutter *cell* per height, not one column for the whole panel.
    # No column is clear end to end in either client: the reference's top card
    # reaches within 10px of the panel edge, and this shell's panel carries a
    # scrollbar in the outer 10px with cards filling the rest, so a column can be
    # background at the top and card at the bottom or the other way about. A cell
    # is the panel's own surface only where it is darker than the chrome it
    # frames *and* not greyer than it -- which is the property under test,
    # checked before the sample is used.
    chrome_lum = lum(REF["chrome"])

    def strip(x, y0):
        vals = [px(x, y) for y in range(y0 - 6, y0 + 7)]
        return tuple(sorted(v[i] for v in vals)[len(vals) // 2] for i in range(3))

    cells: list[tuple[int, int, tuple[int, int, int]]] = []
    for frac in (0.10, 0.22, 0.34, 0.46, 0.58, 0.70, 0.82, 0.92):
        y = bar_bottom + int((h - bar_bottom) * frac)
        for x in range(w - 3, w - 22, -1):
            c = strip(x, y)
            if lum(c) <= chrome_lum - 4 and c[1] >= c[2]:
                cells.append((y, x, c))
                break

    if not cells:
        # Not an infrastructure error: this is the tint gate failing, reached by
        # its own precondition. The build this replaced had no cell darker than
        # its chrome, because its panel *was* the chrome's colour.
        print(
            "  [FAIL] panel gutter is a dark brand tint, not the raised grey: "
            "no cell in the panel's right margin is darker than the chrome, so "
            "the panel is painted the chrome's own surface"
        )
        print("\npanel gate FAILED (1): panel gutter is a dark brand tint")
        return 1

    top = cells[0][2]
    bottom = cells[-1][2]
    panel_bg = tuple(
        sorted(c[2][i] for c in cells)[len(cells) // 2] for i in range(3)
    )
    print(f"panel gutter: {len(cells)} of 8 heights have a clear cell")
    for y, x, c in cells:
        print("  y=%-4d x=%-4d #%02x%02x%02x (w-%d)" % ((y, x) + c + (w - x,)))

    g.check(
        150 < (w - panel_left) < 400,
        "panel is a panel's width",
        f"{w - panel_left}px of {w} (reference: 237 of 1088, i.e. 300 logical)",
    )

    print("gutter top #%02x%02x%02x  bottom #%02x%02x%02x" % (top + bottom))

    g.check(
        len(cells) >= 4,
        "the panel's gutter is clear for most of its height",
        f"{len(cells)} of 8 sampled heights (want >= 4)",
    )

    # G1: a dark brand tint, not the raised grey. Stated as luminance against the
    # chrome *and* green at or above blue, because green-minus-blue alone is a
    # one-level difference on surfaces this dark and would be asserting the
    # arithmetic rather than the appearance. Both halves separate the two builds
    # unambiguously: the raised grey measures green five levels below blue and
    # the same luminance as the chrome it frames, while the tint sits 6-7 levels
    # darker with green at or above blue.
    g.check(
        top[1] >= top[2] and lum(top) <= chrome_lum - 4,
        "panel gutter is a dark brand tint, not the raised grey",
        ("#%02x%02x%02x: green-blue %+d, luminance %.1f vs chrome %.1f "
         "(want green >= blue, and >= 4 below chrome)")
        % (top + (top[1] - top[2], lum(top), chrome_lum)),
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

    # G9/G10: the brand's two fills, searched rather than sampled.
    #
    # The accent is the fill of whatever primary button a page happens to draw
    # and the plate is behind whichever rail entry is active, so both move with
    # the page and a fixed coordinate would only ever measure one page. What does
    # not move is that each is a *flat fill* -- thousands of pixels of one colour
    # -- while an antialiased edge of the same hue is a rounding error, so the
    # modal colour of the pixels that pass a hue test finds them and a threshold
    # would not.
    def modal_where(predicate):
        counts: dict[tuple[int, int, int], int] = {}
        for y in range(0, h, 2):
            for x in range(0, w, 2):
                c = px(x, y)
                if predicate(c):
                    counts[c] = counts.get(c, 0) + 1
        if not counts:
            return None, 0
        best = max(counts, key=counts.get)
        return best, counts[best]

    accent_px, accent_n = modal_where(
        lambda c: c[1] > 140 and c[1] - c[0] > 60 and c[1] - c[2] > 40
    )
    print(
        "accent fill #%02x%02x%02x over %d sampled px" % ((accent_px or (0, 0, 0)) + (accent_n,))
    )
    g.check(
        accent_px is not None and near(accent_px, REF["accent"], 6),
        "the accent is the brand green the app paints",
        "#%02x%02x%02x against #%02x%02x%02x" % ((accent_px or (0, 0, 0)) + REF["accent"]),
    )

    # The plate is a dark green: green above blue above red, and all three inside
    # the band the two measured surfaces occupy. A selected card's 6% tint
    # (`#25` green ~52) and the panel's wash (green 34) both fall outside it, so
    # the only thing that answers is the plate itself.
    plate_px, plate_n = modal_where(
        lambda c: c[0] < 48 and 60 <= c[1] <= 110 and 40 <= c[2] <= 92 and c[2] > c[0]
    )
    print(
        "active plate #%02x%02x%02x over %d sampled px" % ((plate_px or (0, 0, 0)) + (plate_n,))
    )
    g.check(
        plate_px is not None and near(plate_px, REF["plate"], 8),
        "the active plate is the brand highlight",
        "#%02x%02x%02x against #%02x%02x%02x" % ((plate_px or (0, 0, 0)) + REF["plate"]),
    )

    # G11: the plate behind the active rail entry is a *circle*, not a rounded
    # rectangle.
    #
    # The colour assertion above cannot see this and said the right thing about
    # the wrong shape for a release. `NavButton.vue` is `w-12 h-12 rounded-full`
    # whose selected state is a `::before` at `inset: 0` with `border-radius:
    # 50%`, and the reference's own window draws exactly that -- but this shell
    # drew a 12px radius, which is what a reading of the plate's corner arc had
    # said, and the two agree on the fill and on the widest row and disagree
    # everywhere else.
    #
    # Row width is what separates them, so it is what is measured. Across a 48px
    # plate:
    #
    #     circle, r=24      10, 26, 30, ... 48 (widest, 7 rows either side of the
    #                       centre), ... 30, 26, 12
    #     rounded, r=12     24, 26, 34, ... 48 (widest, 12 rows either side), ...
    #
    # i.e. a circle's top row is about a quarter of its widest and a 12px radius's
    # is about a half. The test is stated as that ratio: the widest row must be the
    # plate's own height and the top row must be under 40% of it. A square plate
    # fails both, a 12px radius fails the second, and only a circle passes.
    # The rows are counted against the plate's *own* colour rather than against
    # the hue band above, which is what the first attempt at this did and what it
    # got wrong: the band is wide enough to also admit the antialiased edge of the
    # logo in the head and the plate's own icon, so the box it measured ran from
    # y 9 to y 91 and its widest row was the icon. The colour the assertion above
    # just agreed on is exact, so the shape is counted at that colour.
    # The plate's colour, taken from the modal fill above when there is one and
    # from the dark-green band otherwise: the two reference captures this gate is
    # run against disagree about which entry is active (the walk's Home capture
    # has Home selected, its Screenshots capture has the page open before the rail
    # repaints), and a shape assertion should not depend on that. Both candidates
    # are flat fills of the same component, so reading either is reading the
    # plate; what is *not* read is the icon on it, which is the accent and far
    # brighter.
    candidates = [
        c
        for c in (plate_px,)
        if c is not None
    ] or [(0x1D, 0x56, 0x3F)]

    def widest_run(y: int) -> tuple[int, int]:
        """The longest horizontal run of plate colour on row `y`, and where it starts."""
        best = (0, 0)
        for cand in candidates:
            run = 0
            for x in range(0, w):
                if near(px(x, y), cand, 6):
                    run += 1
                    if run > best[0]:
                        best = (run, x - run + 1)
                else:
                    run = 0
        return best

    rows = {y: widest_run(y) for y in range(0, h)}
    # A plate row is one with a run of at least a quarter of the plate's own
    # width; the antialiased scatter of the same colour elsewhere is a few pixels.
    solid = [y for y, (n, _) in rows.items() if n >= 12]
    if not solid:
        g.check(False, "the plate is a circle, not a rounded rectangle", "no plate found")
        g.check(False, "the plate is as wide as it is tall", "no plate found")
    else:
        top, bottom = min(solid), max(solid)
        left = min(rows[y][1] for y in solid)
        right = max(rows[y][1] + rows[y][0] - 1 for y in solid)
        widths = {y: rows[y][0] for y in range(top, bottom + 1)}
        widest_y = max(widths, key=widths.get)
        widest = widths[widest_y]
        # The entry is inset 8px in a 64px rail, so a 48px plate runs x 8..55.
        # The corner rows carry the same antialiasing as everything else here, so
        # the ratio is read off the first row that has any plate in it at all.
        first = widths[top]
        print(
            "active plate rows: top %d px at y %d, widest %d px at y %d, "
            "bottom %d px at y %d (box x %d..%d, y %d..%d)"
            % (first, top, widest, widest_y, widths[bottom], bottom, left, right, top, bottom)
        )
        tall = bottom - top + 1
        g.check(
            widest >= tall * 0.85,
            "the plate is as wide as it is tall",
            f"widest row {widest}px against a {tall}px height",
        )
        # The discriminating number, and the only one the two shapes do not share.
        # The middle row is not asserted: the box is the rows with a *solid* run of
        # the plate's colour, which stops a row or two short of the plate's
        # geometric top and bottom, so `widest_y` against the box's middle is a
        # statement about the threshold rather than about the plate. The top row's
        # share of the widest row is not: both shapes have the same widest row and
        # the same rows counted, so the ratio between them is the shape.
        g.check(
            first * 100 < widest * 40,
            "the plate is a circle, not a rounded rectangle",
            f"top row {first}px = {first * 100 // max(widest, 1)}% of the widest "
            f"{widest}px (a circle is ~30%, a 12px radius 50%), widest at y {widest_y}",
        )

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
