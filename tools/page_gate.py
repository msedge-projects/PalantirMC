"""Assert that a page's own content matches the reference client, off one capture.

`panel_gate.py` judges the shell -- the rail, the page pane, the right panel. This
judges what a *page* draws inside that pane, because the port is per page: the
Screenshots page first, then the rest, each one against numbers measured off the
Modrinth App's own window rather than estimated from a screenshot of it.

    python tools/page_gate.py .scratch/pal-shots.png
    python tools/page_gate.py .scratch/pal-home.png --page home

Exits 0 and prints `page gate passed` only when every assertion holds.

Every number below came out of the reference at a pinned 1280x720 client, at a
device scale of 1.0, and is recorded with its sample in `REFERENCE.md`:

    illustration   x 409..624, y 296..408   (216x113, fill #1d1f23, frame #34363c)
    heading        ink rows 462..484, cap 18, colour #ffffff
    subtext        ink rows 497..512, cap 12, colour #95a2af
    column         x 65..979, page top y 49 (a 1px #42444a rule above it)
    centring       content centre x 516.5 = the column's middle (522.5) minus the
                   11px scrollbar gutter, and y 404.5 = its middle (384.5) + 20

Each assertion is stated so that the build it replaced fails it, which is the only
thing that makes a passing run mean anything. Both controls are recorded:

    * `.scratch/ref-07-still-1.png` -- the reference itself; must pass
    * `.scratch/pal-shots-old.png`  -- the build before this one; must fail, and by
      the first gate: it drew an in-page "Screenshots" heading, a rule and a
      Refresh chip, so its page column has more than one cluster in it

Captures are `PrintWindow` grabs from `tools/winshot.py`, taken off-screen and
without activating the window. `.scratch/ref-07-still-1.png` is not committed --
it is a screenshot of another product -- so the reference half of the control is a
recorded manual result, and the numbers below are what it measured.
"""
from __future__ import annotations

import sys
from pathlib import Path

try:
    from PIL import Image
except ImportError:  # pragma: no cover - environment guard, not logic
    print("Pillow is required: python -m pip install pillow")
    raise SystemExit(2)

sys.path.insert(0, str(Path(__file__).resolve().parent))
import refsample  # noqa: E402  (Windows OCR, the same reader the walk uses)
from panel_gate import (  # noqa: E402
    INSET,
    Gate,
    crop_window_frame,
    horizontal_edge,
    vertical_edge,
)

# The pages this gate knows, and the name each puts in the title bar. The bar's
# label is the page's own name (`REFERENCE.md`): Home reads "Home" and
# Screenshots reads "Screenshots", which is also the entire heading that page has.
PAGES = {"home": "Home", "screenshots": "Screenshots"}

# ---- The reference's own page, measured from its capture -------------------
REF = {
    "page": (0x16, 0x18, 0x1C),
    "art_fill": (0x1D, 0x1F, 0x23),   # the illustration's frames
    "art_frame": (0x34, 0x36, 0x3C),  # their outline
    "heading": (0xFF, 0xFF, 0xFF),    # 24px bold, --color-text-primary
    "subtext": (0x95, 0xA2, 0xAF),    # 16px, --color-text-tertiary
    "art_box": (216, 113),
    # Ink-to-ink, which is what a capture can be asked about without knowing
    # where a text widget's box begins: the art's lowest ink row (408) to the
    # heading's highest (462), then the heading's to the subtext's (497).
    "art_to_heading": 54,
    "heading_to_subtext": 35,
    # The ink box, not the cap: a capture shows where a glyph's ink lands, and
    # both lines have descenders ("yet", "in-game"). The heading's ink is 23 rows
    # (cap 18 with 5 rows of descender under it) and the subtext's is 16 (cap 12
    # with 4) -- which is also how the sizes were identified: 23 rows of ink at
    # Inter's metrics is a 24px line, 16 is a 16px one.
    "heading_ink_rows": 23,
    "subtext_ink_rows": 16,
    "gutter": 11,      # the scrollbar band the content is centred inside
    "centre_drop": 20,  # how far below the column's middle the block sits
}

# The bar's own name: 16px semibold white, the same size the reference sets it.
BAR_HEAD = 48

TOL = 6
# Ink is counted at a threshold that separates it from the page it sits on, and
# the illustration is counted at a lower one: its frames are filled with
# `#1d1f23`, one level off the page's `#16181c`, so only their outline and their
# sun are strong enough for the higher threshold -- and counting the outline alone
# measures the frame, not the box the page reserves for it.
INK = 120
SHAPE = 12


def diff(a, b):
    return sum(abs(int(x) - int(y)) for x, y in zip(a, b))


def near(c, ref, tol=TOL):
    return all(abs(int(a) - int(b)) <= tol for a, b in zip(c, ref))


def rows_with_content(im, x0, x1, y0, y1, bg, thr):
    """Every row in `y0..y1` holding a pixel more than `thr` away from `bg`."""
    pix = im.load()
    rows = []
    for y in range(y0, y1):
        for x in range(x0, x1):
            if diff(pix[x, y], bg) > thr:
                rows.append(y)
                break
    return rows


def bands(rows, merge=3):
    """Contiguous runs of rows, allowing a `merge`px gap between them."""
    out: list[list[int]] = []
    for y in rows:
        if out and y - out[-1][1] <= merge:
            out[-1][1] = y
        else:
            out.append([y, y])
    return [tuple(b) for b in out]


def columns_in(im, x0, x1, y0, y1, bg, thr):
    """The x extent of the content in a band."""
    pix = im.load()
    lo, hi = None, None
    for x in range(x0, x1):
        for y in range(y0, y1 + 1):
            if diff(pix[x, y], bg) > thr:
                lo = x if lo is None else lo
                hi = x
                break
    return lo, hi


def first_strong_edge(im, lo, hi, frac=0.6):
    """The first *nearly* strongest vertical boundary in `[lo, hi)`.

    `panel_gate.vertical_edge` answers with the strongest one, and for this one
    boundary that is the wrong question: the panel carries a scrollbar in its outer
    band, the window carries a resize grip outside that, and both are harder edges
    than the panel's own left side -- a brand wash against a page, a few levels
    apart. The strongest boundary therefore lands *inside* the panel and the "page
    column" comes back as page-plus-panel, whose cards put ink in every row and
    hide the page's own bands. The first boundary that is within `frac` of the
    strongest is the page's right edge in both clients, which is what this gate
    is about.
    """
    w, h = im.size
    ys = range(2, h - 2, 3)
    scores = {}
    for x in range(max(lo, 1), min(hi, w)):
        scores[x] = sum(
            1 for y in ys if diff(im.getpixel((x, y)), im.getpixel((x - 1, y))) > 6
        )
    if not scores:
        return None, 0
    best = max(scores.values())
    for x in sorted(scores):
        if scores[x] >= best * frac:
            return x, scores[x]
    return None, best


def page_bottom(im, x0, x1, top, page):
    """The last row of the page column: where the page colour stops.

    Found by colour rather than by assuming the column runs to the window's edge,
    because that is the difference between the two clients: the reference's page
    column does run to the bottom, and this shell's is cut short by its own status
    strip, which is chrome with a 1px rule above it. One gate has to judge both, so
    it measures the run of page colour that starts under the bar and ends where
    that colour does. (Below the status strip the window's resize grip is painted
    in the page's own colour again, which is exactly why the run is followed from
    the top rather than searched for from the bottom.)
    """
    pix = im.load()
    last = top
    for y in range(top, im.height):
        hits = sum(1 for x in range(x0, x1, 3) if diff(pix[x, y], page) <= 6)
        # Mostly page colour, not entirely: the column's content is narrow -- the
        # illustration is 216 of its 900-odd columns -- so a row through it is still
        # three-quarters page, while the first row of the shell's status strip is
        # none of it.
        if hits < (x1 - x0) // 3 * 0.5:
            break
        last = y
    return last


def modal(im, x0, x1, y0, y1, bg, thr):
    """The commonest colour at least `thr` from `bg` inside a band.

    A modal colour rather than a sample, for the reason `panel_gate`'s accent gate
    is modal: a glyph's ink is thousands of pixels of one colour, while the
    antialiased edge of the same ink is a rounding error.
    """
    pix = im.load()
    counts: dict[tuple[int, int, int], int] = {}
    for y in range(y0, y1 + 1):
        for x in range(x0, x1):
            c = pix[x, y]
            if diff(c, bg) > thr:
                counts[c] = counts.get(c, 0) + 1
    if not counts:
        return None, 0
    best = max(counts, key=counts.get)
    return best, counts[best]


def main() -> int:
    argv = sys.argv[1:]
    only = None
    page = "screenshots"
    if "--only" in argv:
        i = argv.index("--only")
        only = argv[i + 1]
        del argv[i : i + 2]
    if "--page" in argv:
        i = argv.index("--page")
        page = argv[i + 1]
        del argv[i : i + 2]
    if len(argv) != 1 or page not in PAGES:
        print(__doc__)
        return 2

    path = Path(argv[0])
    if not path.exists():
        print(f"capture not found: {path}")
        return 2

    # G17 first, and off the uncropped capture: it is the one assertion that does
    # not need the columns, so a page whose column cannot be found still reports
    # where its name is. Read by OCR rather than by ink, because "the bar names
    # the page" is a claim about words.
    g = Gate(only)
    scope = f" [{only}]" if only else ""
    want = PAGES[page]
    named = [
        line
        for line in refsample.ocr(path)
        if line["y"] < BAR_HEAD and line["text"].strip().strip("|").strip() == want
    ]
    g.check(
        bool(named),
        "[barname] the title bar names the page it is showing",
        f"read {' | '.join(l['text'] for l in named) if named else 'nothing'} "
        f"where {want!r} was expected (bar labels: "
        f"{', '.join(repr(l['text']) for l in refsample.ocr(path) if l['y'] < BAR_HEAD)})",
    )

    im, frame = crop_window_frame(Image.open(path).convert("RGB"))
    w0, h0 = im.size
    im = im.crop((INSET, INSET, w0 - INSET, h0 - INSET))
    w, h = im.size
    print(
        f"capture {path.name}: {w0}x{h0}"
        + (f" (cropped a {frame}px frame)" if frame else "")
        + f" -> {w}x{h} after a {INSET}px inset"
    )

    # The page column, found rather than assumed: the rail's right edge on the
    # left, the panel's left edge on the right, exactly as `panel_gate` locates
    # them, so a window of another size still measures the same column.
    pane_left, pane_score = vertical_edge(im, int(w * 0.02), int(w * 0.2))
    # The panel's *left* edge, which is the first strong boundary on that side
    # rather than the strongest -- see `first_strong_edge` for why that distinction
    # is the difference between measuring the page and measuring the page plus the
    # panel.
    panel_left, panel_score = first_strong_edge(im, int(w * 0.62), w - 2)
    if pane_left is None or panel_left is None:
        print("\npage gate FAILED: the column's boundaries were not found")
        return 1
    # Six pixels in from each boundary, not two: the columns' own edges are a
    # gradient between the surfaces (the rail's chrome into the page, the page into
    # the panel's wash), and those transition columns are ink at every row -- which
    # reads as one band from the top of the page to the bottom and hides the three
    # that are actually there.
    x0, x1 = pane_left + 6, panel_left - 6
    # The column's own background: the modal colour of it, which is the page
    # because a page is mostly empty.
    page, page_n = modal(im, x0, x1, 0, h - 1, (0, 0, 0), 0)
    print(
        f"column x {pane_left}..{panel_left} (scores {pane_score}/{panel_score}), "
        f"page #{page[0]:02x}{page[1]:02x}{page[2]:02x} over {page_n} px"
    )
    g.check(
        near(page, REF["page"], 3),
        "[page] the column is the reference's page colour",
        "#%02x%02x%02x against #%02x%02x%02x" % (page + REF["page"]),
    )

    # The scan starts below the page's own top edge, found rather than assumed:
    # the pane's top-left corner is cut (that is `panel_gate`'s G5) and the bar's
    # rule and that arc are ink too, so a scan from the capture's first row would
    # count them as content. The margin below the edge is the corner's own radius.
    bar_bottom, bar_score = horizontal_edge(im, int(h * 0.04), int(h * 0.16))
    if bar_bottom is None:
        print("\npage gate FAILED: the page's top edge was not found")
        return 1
    top = bar_bottom + 20
    bottom = page_bottom(im, x0, x1, top, page)
    print(
        f"page top y={bar_bottom} (score {bar_score}), scanning from y={top} "
        f"to y={bottom} where the page colour ends"
    )
    found = bands(rows_with_content(im, x0, x1, top, bottom, page, SHAPE))
    print("content bands: " + ", ".join(f"y {b[0]}..{b[1]}" for b in found))

    # G11: one cluster, and only one. The reference's Screenshots page draws no
    # heading, no rule and no control -- its title is in the title bar -- so a
    # second cluster here means the page has grown chrome of its own again.
    g.check(
        len(found) == 3,
        "[cluster] the column holds one cluster and nothing else",
        f"{len(found)} bands" + (" (the replaced build drew an in-page heading, a rule and a Refresh chip)" if len(found) > 3 else ""),
    )
    if len(found) != 3:
        # Everything below measures the three bands, so there is nothing to say
        # about them here -- but the verdict still has to be the whole run's, not
        # this early exit's: scoped to a gate that needs the bands, "nothing ran"
        # is not a pass, and scoped to one that does not, it is not a failure.
        print()
        if g.failures:
            print(f"page gate FAILED{scope} ({len(g.failures)}): " + ", ".join(g.failures))
            return 1
        if not g.ran:
            print(f"no assertion matched --only {only!r} (it needs the three bands)")
            return 2
        print(f"page gate passed{scope}")
        return 0
    (art_y0, art_y1), (head_y0, head_y1), (sub_y0, sub_y1) = found

    art = columns_in(im, x0, x1, art_y0, art_y1, page, SHAPE)
    head = columns_in(im, x0, x1, head_y0, head_y1, page, INK)
    sub = columns_in(im, x0, x1, sub_y0, sub_y1, page, INK)
    art_box = (art[1] - art[0] + 1, art_y1 - art_y0 + 1)
    head_ink = head_y1 - head_y0 + 1
    sub_ink = sub_y1 - sub_y0 + 1
    print(
        f"illustration x {art[0]}..{art[1]} ({art_box[0]}x{art_box[1]}), "
        f"heading x {head[0]}..{head[1]} ink {head_ink} rows, "
        f"subtext x {sub[0]}..{sub[1]} ink {sub_ink} rows"
    )

    # G12: the illustration's box is the one the page reserves.
    g.check(
        abs(art_box[0] - REF["art_box"][0]) <= 8 and abs(art_box[1] - REF["art_box"][1]) <= 8,
        "[box] the illustration fills the reference's 216x113 box",
        f"{art_box[0]}x{art_box[1]} against {REF['art_box'][0]}x{REF['art_box'][1]}",
    )
    fill, fill_n = modal(im, art[0], art[1] + 1, art_y0, art_y1, page, 2)
    outline, outline_n = modal(im, art[0], art[1] + 1, art_y0, art_y1, page, 60)
    g.check(
        fill is not None and near(fill, REF["art_fill"], 4)
        and outline is not None and near(outline, REF["art_frame"], 4),
        "[colours] the illustration is drawn in the reference's own two colours",
        "#%02x%02x%02x fill (%d px) and #%02x%02x%02x outline (%d px) against "
        "#%02x%02x%02x and #%02x%02x%02x"
        % (fill + (fill_n,) + outline + (outline_n,) + REF["art_fill"] + REF["art_frame"])
        if fill and outline
        else "no fill or outline found",
    )

    # G13/G14: the two lines are at the reference's sizes and colours.
    head_ink_color, _ = modal(im, head[0], head[1] + 1, head_y0, head_y1, page, INK)
    g.check(
        head_ink_color is not None
        and near(head_ink_color, REF["heading"], 6)
        and abs(head_ink - REF["heading_ink_rows"]) <= 2,
        "[heading] the heading is the reference's 24px bold white",
        "#%02x%02x%02x in %d ink rows against #%02x%02x%02x in %d"
        % (head_ink_color + (head_ink,) + REF["heading"] + (REF["heading_ink_rows"],)),
    )
    sub_ink_color, _ = modal(im, sub[0], sub[1] + 1, sub_y0, sub_y1, page, INK)
    g.check(
        sub_ink_color is not None
        and near(sub_ink_color, REF["subtext"], 6)
        and abs(sub_ink - REF["subtext_ink_rows"]) <= 2,
        "[subtext] the subtext is the reference's 16px tertiary",
        "#%02x%02x%02x in %d ink rows against #%02x%02x%02x in %d"
        % (sub_ink_color + (sub_ink,) + REF["subtext"] + (REF["subtext_ink_rows"],)),
    )

    # G15: the gaps between them, ink row to ink row.
    gap_a = head_y0 - art_y1
    gap_b = sub_y0 - head_y0
    print(f"gaps: illustration->heading {gap_a}, heading->subtext {gap_b}")
    g.check(
        abs(gap_a - REF["art_to_heading"]) <= 6 and abs(gap_b - REF["heading_to_subtext"]) <= 5,
        "[gaps] the gaps between them are the reference's",
        f"{gap_a} and {gap_b} against {REF['art_to_heading']} and {REF['heading_to_subtext']}",
    )

    # G16: centred on the column's content box, not its border box, and 20px
    # below its middle.
    # The column spans from the page's first row under the bar's rule to the
    # capture's last, and the content is centred inside it minus the gutter it
    # ignores -- so both centres are half-way along that box, the vertical one
    # dropped by the 20px the reference's taller content box produces.
    content_centre = (art[0] + art[1]) / 2
    column_top, column_bottom = bar_bottom + 1, bottom
    want_x = (pane_left + panel_left - REF["gutter"]) / 2
    block_centre = (art_y0 + sub_y1) / 2
    want_y = (column_top + column_bottom) / 2 + REF["centre_drop"]
    print(
        f"centre x {content_centre:.1f} against {want_x:.1f}; "
        f"y {block_centre:.1f} against {want_y:.1f}"
    )
    g.check(
        abs(content_centre - want_x) <= 6 and abs(block_centre - want_y) <= 8,
        "[centring] the block is centred where the reference centres it",
        f"({content_centre:.1f}, {block_centre:.1f}) against ({want_x:.1f}, {want_y:.1f})",
    )

    print()
    if not g.ran:
        print(f"no assertion matched --only {only!r}")
        return 2
    if g.failures:
        print(f"page gate FAILED{scope} ({len(g.failures)}): " + ", ".join(g.failures))
        return 1
    print(f"page gate passed{scope}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
