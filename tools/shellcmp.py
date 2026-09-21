"""Compare the shell's frame between the reference client and this one, in one run.

    python tools/shellcmp.py .scratch/ref-01-home.png=REFERENCE \
                            .scratch/port-ours-shots4.png=OURS

Every other gate measures one window against recorded numbers. This measures two
windows against each other, and it exists because there is a class of mistake
neither of them can see alone: a part that is drawn to a plausible number in both
clients but is *offset* in one of them. This shell had exactly that. It wrapped
its window in a 6px band of resize grips on every edge and painted the top one in
the page's colour, so its head began at y 6 and its rule sat at y 54 where the
reference's head begins at y 0 and its rule is at y 48 -- six pixels, in every
vertical measurement in the window, invisible to any gate that looked at one
window at a time.

What it reports is deliberately blunt -- the y of the rule, the pane's corner rows
and the rail plate's rows, on each capture -- so that a run is readable as a
comparison rather than as a verdict about one file. Each field carries a
tolerance, because the two captures do not come from the same kind of tool: the
reference's are `PrintWindow` grabs and ours are written by the launcher from its
own frame, so antialiased edges land a level apart and a 1px ring can read as 28 on
one side and 30 on the other.

Exits 0 and prints `shell comparison passed` only when every field agrees. The
captures are not committed -- one of them is a screenshot of another product --
so the command is the record and the numbers live in `GATES.md`.
"""
from __future__ import annotations

import sys
from pathlib import Path

try:
    from PIL import Image
except ImportError:  # pragma: no cover - environment guard, not logic
    print("Pillow is required: python -m pip install pillow")
    raise SystemExit(2)

# The three surfaces the frame is built from, as the reference's own window paints
# them: the raised chrome, the page, and the 1px rule under the head.
CHROME = (0x27, 0x29, 0x2E)
PAGE = (0x16, 0x18, 0x1C)
RULE = (0x42, 0x44, 0x4A)
# The active plate, which `REFERENCE.md` records as `#1d563f` in the reference's
# rail and `#1d5540` in its tab strip -- one level apart, on two surfaces.
PLATE = (0x1D, 0x56, 0x3F)

# How far apart the two clients may be, in px. A capture of the reference carries a
# native frame that ours does not, and the tools that produced them differ, so the
# number is not 0 -- but it is small enough that the 6px offset this tool was
# written for fails every field it touches.
TOL = 2


def near(p, c, tol=4):
    return all(abs(a - b) <= tol for a, b in zip(p[:3], c))


def sample(im, label):
    """The frame's own landmarks, measured the same way on either capture."""
    out = {"label": label}

    # The rule under the head: the only row in the top eighth that is mostly the
    # rule's colour across the page column's width.
    out["rule_y"] = None
    for y in range(0, min(120, im.height)):
        hits = sum(1 for x in range(200, 1000, 20) if near(im.getpixel((x, y)), RULE, 6))
        if hits >= 35:
            out["rule_y"] = y
            break

    # The pane's corner: the first page-coloured row at four x positions, which is
    # the arc of its 20px radius read along the top.
    base = (out["rule_y"] or 48) + 1
    out["corner"] = []
    for x in (66, 72, 80, 88):
        got = next(
            (y for y in range(base, base + 40) if near(im.getpixel((x, y)), PAGE, 3)),
            None,
        )
        out["corner"].append(got)

    # The plate: the widest run of its colour over the rail's column, and the run
    # on the first and last row that has one at all.
    # Each row's *own* longest run, not the best seen so far: the plate's shape is
    # the sequence of row widths, so a running maximum would report the widest row
    # for every row in the plate and there would be nothing left to compare.
    rows: dict[int, int] = {}
    for y in range(0, im.height):
        run = best = 0
        for x in range(2, 62):
            if near(im.getpixel((x, y)), PLATE, 10):
                run += 1
                best = max(best, run)
            else:
                run = 0
        rows[y] = best
    widest = max(rows.values()) if rows else 0
    # Only the rows that belong to the widest run's own band, so the plate's rows
    # are the plate and not every antialiased edge of the same hue in the rail.
    band = [y for y, n in rows.items() if n >= widest * 0.25]
    if band and widest:
        top, bottom = min(band), max(band)
        out["plate_rows"] = (rows[top], widest, rows[bottom])
        out["plate_box"] = (top, bottom)
    else:
        out["plate_rows"] = None
        out["plate_box"] = None
    return out


def main() -> int:
    if len(sys.argv) < 3:
        print(__doc__)
        return 2

    shots = {}
    for arg in sys.argv[1:]:
        path, _, label = arg.partition("=")
        p = Path(path)
        if not p.exists():
            print(f"missing capture: {p}")
            return 2
        shots[label or p.stem] = sample(Image.open(p).convert("RGB"), label or p.stem)

    names = list(shots)
    ref, ours = shots[names[0]], shots[names[-1]]
    print(f"comparing {names[0]} against {names[-1]}")
    print(f"  rule under the head   {ref['label']} y {ref['rule_y']}   {ours['label']} y {ours['rule_y']}")
    print(f"  pane corner rows      {ref['label']} {ref['corner']}   {ours['label']} {ours['corner']}")
    print(
        f"  rail plate (top,widest,bottom)  {ref['label']} {ref['plate_rows']}"
        f"   {ours['label']} {ours['plate_rows']}"
    )

    bad = []
    if ref["rule_y"] is None or ours["rule_y"] is None:
        bad.append("the rule under the head was not found on one of the captures")
    elif abs(ref["rule_y"] - ours["rule_y"]) > TOL:
        bad.append(f"the rule is at y {ref['rule_y']} there and y {ours['rule_y']} here")

    for i, x in enumerate((66, 72, 80, 88)):
        a, b = ref["corner"][i], ours["corner"][i]
        if a is None or b is None:
            bad.append(f"the pane's corner was not found at x {x}")
        elif abs(a - b) > TOL:
            bad.append(f"the pane's corner row at x {x} is {a} there and {b} here")

    if ref["plate_rows"] is None or ours["plate_rows"] is None:
        bad.append("the active plate was not found on one of the captures")
    else:
        for i, what in enumerate(("top", "widest", "bottom")):
            a, b = ref["plate_rows"][i], ours["plate_rows"][i]
            if abs(a - b) > max(TOL, 2):
                bad.append(f"the plate's {what} row is {a}px there and {b}px here")
        # The one field where the two shapes part company, restated here so this
        # tool fails a rounded rectangle on either client rather than only a
        # difference between them.
        for s in (ref, ours):
            top, widest, _ = s["plate_rows"]
            if widest and top * 100 >= widest * 40:
                bad.append(
                    f"{s['label']}'s plate is not a circle: top row {top}px is "
                    f"{top * 100 // widest}% of the widest {widest}px"
                )

    print()
    if bad:
        print(f"shell comparison FAILED ({len(bad)}):")
        for b in bad:
            print(f"  - {b}")
        return 1
    print("shell comparison passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
