"""Assert that a page's own content matches the reference client, off one capture.

`panel_gate.py` judges the shell -- the rail, the page pane, the right panel. This
judges what a *page* draws inside that pane, because the port is per page: the
Screenshots page first, then Home, each one against numbers measured off the
Modrinth App's own window rather than estimated from a screenshot of it.

    python tools/page_gate.py .scratch/pal-shots.png
    python tools/page_gate.py .scratch/pal-home.png --page home

Both halves of every control are on disk, at the same client size, from the same
command -- `python tools/appshot.py --page X --out Y` for this launcher and the
capture the walk took for the reference:

    reference   .scratch/ref-01-home.png     passes (12/12)
    ours        .scratch/pal-home-new.png    passes (12/12)
    the build this port replaces
                .scratch/pal-home-oldctrl.png  fails on the first assertion: the
                old page drew one card holding everything, so its column has one
                band where the reference's has seven

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

A capture of *this* launcher comes from the launcher: iced hands the process its
own frame, so `--shot` writes the pixels the shell drew rather than the
compositor's opinion of a window parked off the desktop. `tools/appshot.py` drives
it. The reference's captures are `PrintWindow` grabs from `tools/winshot.py`, taken
off-screen and without activating the window. None of those files are committed --
they are screenshots of another product, and two of them are of this one before a
port -- so the reference half of each control is a recorded result, and the numbers
below are what it measured.
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


def core(im, x0, x1, y0, y1, bg, slack=12):
    """The commonest colour among a band's *strongest* ink.

    `modal` answers for a solid fill and mis-answers for thin type: a 16px regular
    line on the reference's dotted backdrop is all antialiasing, and its
    most-common colour at any threshold is a half-lit edge -- `#34383d` against a
    token of `#b0bac5`. Comparing only against the brightest ink, within `slack`
    levels of the band's strongest pixel, asks the question the assertion is
    actually about -- what colour the glyph's core is drawn in -- and answers
    `#afbac4` for that line, one level off its token. Neither the reference's
    WebView nor this toolkit draws type as a field of one colour; a tolerance is
    part of the measurement, not a kindness to it.
    """
    pix = im.load()
    lit = [
        (diff(pix[x, y], bg), pix[x, y])
        for y in range(y0, y1 + 1)
        for x in range(x0, x1)
        if diff(pix[x, y], bg) > 12
    ]
    if not lit:
        return None, 0
    bar = max(d for d, _ in lit) - slack
    counts: dict[tuple[int, int, int], int] = {}
    for d, c in lit:
        if d >= bar:
            counts[c] = counts.get(c, 0) + 1
    if not counts:
        return None, 0
    best = max(counts, key=counts.get)
    return best, counts[best]


def darkest(im, x0, x1, y0, y1, ceiling=100):
    """The commonest *near-black* colour in a band.

    For a label drawn on a bright fill, brightness is the wrong direction to look:
    the reference's Import button is `#34363c` with `#b0bac5` on it, but its brand
    button is `#00da75` with `#000000`, and black is only 74 levels from the page --
    below every threshold that finds the green around it. So this asks the other
    question: of the pixels that are nearly black, which colour is there most?
    """
    pix = im.load()
    counts: dict[tuple[int, int, int], int] = {}
    for y in range(y0, y1 + 1):
        for x in range(x0, x1):
            c = pix[x, y]
            if sum(c) <= ceiling:
                counts[c] = counts.get(c, 0) + 1
    if not counts:
        return None, 0
    best = max(counts, key=counts.get)
    return best, counts[best]


def dense_rows(im, x0, x1, y0, y1, bg, thr, share):
    """Rows that are mostly ink across `x0..x1`.

    The illustration needs a different question from every other band. Its frames
    are filled with `#1d1f23`, 21 levels from the page -- *below* the reference's
    own dot texture at 57 -- so no threshold sees the artwork's fill without also
    seeing the backdrop. What separates them is not brightness but width: a row of
    the illustration is a hundred ink pixels across, a row of dots is two pixels
    every nine. Requiring most of the band's width to be ink asks exactly that, and
    is why this band's height is measured with a `share` rather than a threshold.
    """
    out = []
    for y in range(y0, y1):
        hits = sum(1 for x in range(x0, x1) if diff(im.getpixel((x, y)), bg) > thr)
        if hits >= share * (x1 - x0):
            out.append(y)
    return out


# ---- Home: the welcome screen ----------------------------------------
#
# Every number is the reference's own, measured off `.scratch/ref-01-home.png`
# and cross-checked against `WelcomeScreen.vue` -- the component that draws it,
# which states the layout in utility classes outright. Where the two agreed, the
# capture is what is recorded here, because ink is what a capture can be asked
# about.
#
#     illustration   y 223..322, ink 96x99 in a 100x100 slot, centre x 516.5
#     title          ink rows 352..371, #ffffff, x 392..640
#     description    ink rows 393..408, #b0bac5, x 431..603
#     brand button   y 435..474 (40 tall), x 414..619, fill #00da75, ink #000000
#     hint row       y 491..510 (20 tall), text #96a2b0, key cap 20x20 at 436..455
#     prompt         ink rows 625..638, #96a2b0
#     import button  y 657..696 (40 tall), x 407..626, #34363c inside #42444a
#     gaps           ink to ink: 30, 22, 27, 17 -- and 19 before the import button
#
# The hero's centring is the one geometry that has to be stated relative to the
# column rather than absolutely, and it is exact in both clients: the reference's
# page column is 49..720 and its hero's centre is 366.5, which is 18 above the
# column's middle; this shell's column is 41px shorter (a status strip and a pane
# inset the reference has no equivalent of) and the same 18 holds, because both
# pin a fixed block 24px off the bottom and grow the hero above it.
HOME = {
    "art_box": 100,
    "title_ink_rows": 20,
    "desc_ink_rows": 16,
    "hint_rows": 20,
    "prompt_ink_rows": 14,
    "title": (0xFF, 0xFF, 0xFF),
    "desc": (0xB0, 0xBA, 0xC5),
    "hint": (0x96, 0xA2, 0xB0),
    "brand_fill": (0x00, 0xDA, 0x75),
    "brand_ink": (0x00, 0x00, 0x00),
    "base_fill": (0x34, 0x36, 0x3C),
    "base_ring": (0x42, 0x44, 0x4A),
    "button_h": 40,
    "cap_box": 20,
    "art_to_title": 30,
    "title_to_desc": 22,
    "desc_to_button": 27,
    "button_to_hint": 17,
    "prompt_to_import": 19,
    "hero_above_middle": 18,
    "paths_found": 7,
}

# Ink at a threshold the reference's own backdrop cannot pass. Its dot texture
# lifts a pixel by at most 20 levels over the page, and the faintest thing this
# page draws -- the illustration's own fill -- is 21 and up, so 60 is above the
# texture and below everything that is a control. Without that margin the 415x478
# grid of dots reads as one band from the art to the top of the page.
HOME_INK = 60


def home_gate(g, im, x0, x1, paper, pane_left, panel_left, top, bottom, scope, only=None) -> int:
    """Judge the Home page's welcome screen.

    `paper` is the page column's own colour and the two `*_left` values are the
    column's boundaries -- the same three things the caller measured to find it.
    Passing them in rather than re-measuring keeps one definition of the column
    shared by both pages.
    """
    found = bands(rows_with_content(im, x0, x1, top, bottom, paper, HOME_INK))
    print("content bands: " + ", ".join(f"y {b[0]}..{b[1]}" for b in found))

    # G20: seven clusters. The reference draws an illustration, a title, a
    # description, a button, a hint row, a prompt and an import button, and
    # nothing else -- no card around them, no rule, no in-page heading. The build
    # this replaced drew all three of those, which is what this notices.
    g.check(
        len(found) == HOME["paths_found"],
        "[clusters] the hero is illustration, title, description, button, hint, "
        "prompt, import -- and nothing else",
        f"{len(found)} bands"
        + (
            " (the replaced build drew a card, an in-page heading, a rule and two chips)"
            if len(found) != HOME["paths_found"]
            else ""
        ),
    )
    if len(found) != HOME["paths_found"]:
        print()
        if g.failures:
            print(f"page gate FAILED{scope} ({len(g.failures)}): " + ", ".join(g.failures))
            return 1
        if not g.ran:
            print(f"no assertion matched --only {only}")
            return 2
        print(f"page gate passed{scope}")
        return 0

    (art0, art1), (t0, t1), (d0, d1), (b0, b1), (h0, h1), (p0, p1), (i0, i1) = found
    art = columns_in(im, x0, x1, art0, art1, paper, HOME_INK)
    title = columns_in(im, x0, x1, t0, t1, paper, HOME_INK)
    desc = columns_in(im, x0, x1, d0, d1, paper, HOME_INK)
    brand = columns_in(im, x0, x1, b0, b1, paper, HOME_INK)
    hint = columns_in(im, x0, x1, h0, h1, paper, HOME_INK)
    prompt = columns_in(im, x0, x1, p0, p1, paper, HOME_INK)
    imp = columns_in(im, x0, x1, i0, i1, paper, HOME_INK)
    # The illustration's footprint: `art` is the x extent its outline gives, and
    # the height comes from the rows that are *mostly* ink across that extent,
    # searched from the band's own top down to where the dots take over.
    art_w = art[1] - art[0] + 1
    dense = dense_rows(im, art[0], art[1] + 1, art0, art1 + 24, paper, 25, 0.3)
    art_h = (dense[-1] - dense[0] + 1) if dense else art1 - art0 + 1
    # The illustration's own bottom, which the gap below it is measured from. Not
    # the band's: at the ink threshold the band ends where the outline does (305),
    # and the artwork continues another eleven rows past it in ink too faint for
    # that threshold -- so measuring the gap from the band would have reported 43
    # where the reference's own rhythm is 30.
    art_bottom = dense[-1] if dense else art1
    print(
        f"illustration {art[0]}..{art[1]} ({art_w}x{art_h}), "
        f"title {title[0]}..{title[1]}, description {desc[0]}..{desc[1]}, "
        f"brand {brand[0]}..{brand[1]}, hint {hint[0]}..{hint[1]}, "
        f"prompt {prompt[0]}..{prompt[1]}, import {imp[0]}..{imp[1]}"
    )

    # G21: the illustration fills the reference's 100px slot.
    g.check(
        abs(art_h - HOME["art_box"]) <= 8,
        "[illustration] the illustration is the reference's 100px square",
        f"{art_w}x{art_h} against {HOME['art_box']}x{HOME['art_box']}",
    )

    # G22/G23: the two lines under it are the reference's sizes and colours.
    title_ink, _ = core(im, title[0], title[1] + 1, t0, t1, paper)
    g.check(
        title_ink is not None
        and near(title_ink, HOME["title"], 6)
        and abs((t1 - t0 + 1) - HOME["title_ink_rows"]) <= 2,
        "[title] the heading is the reference's 24px semibold white",
        "#%02x%02x%02x in %d ink rows against #%02x%02x%02x in %d"
        % (title_ink + (t1 - t0 + 1,) + HOME["title"] + (HOME["title_ink_rows"],)),
    )
    desc_ink, _ = core(im, desc[0], desc[1] + 1, d0, d1, paper)
    g.check(
        desc_ink is not None and near(desc_ink, HOME["desc"], 6),
        "[description] the description is the reference's 16px #b0bac5",
        "#%02x%02x%02x against #%02x%02x%02x" % (desc_ink + HOME["desc"])
        if desc_ink
        else "no ink found",
    )

    # G24: the brand button -- 40px of `#00da75` with black ink on it, which is
    # the pair `--color-brand` and `--color-accent-contrast` resolve to in dark.
    fill, fill_n = modal(im, brand[0], brand[1] + 1, b0, b1, paper, 60)
    ink, ink_n = darkest(im, brand[0] + 8, brand[1] - 8, b0 + 6, b1 - 6)
    g.check(
        fill is not None
        and near(fill, HOME["brand_fill"], 4)
        and ink is not None
        and near(ink, HOME["brand_ink"], 6)
        and abs((b1 - b0 + 1) - HOME["button_h"]) <= 2,
        "[brand button] 40px of the reference's brand, with black ink on it",
        (
            "#%02x%02x%02x (%d px) with #%02x%02x%02x label (%d px) in %d rows "
            "against #%02x%02x%02x / #%02x%02x%02x in %d"
            % (
                fill + (fill_n,) + ink + (ink_n,) + (b1 - b0 + 1,)
                + HOME["brand_fill"] + HOME["brand_ink"] + (HOME["button_h"],)
            )
            if fill and ink
            else "no fill or label found"
        ),
    )

    # G25: the hint row, and the key cap inside it. The cap is found by its own
    # fill rather than by column arithmetic: a column of the cap is eighteen rows
    # of `#34363c`, and a column of the 14px text beside it is two or three of
    # antialiasing. Twelve, not eighteen, because the cap's top and bottom rows are
    # cut by its own corner radius -- asking for the full height measures the flat
    # middle of the cap and reports it as 16 wide when it is 18.
    cap_cols = [
        x
        for x in range(hint[0], hint[1] + 1)
        if sum(1 for y in range(h0, h1 + 1) if diff(im.getpixel((x, y)), HOME["base_fill"]) <= 6) >= 12
    ]
    cap_w = (cap_cols[-1] - cap_cols[0] + 1) if cap_cols else 0
    ring, _ = modal(im, cap_cols[0], cap_cols[-1] + 1, h0, h1, HOME["base_fill"], 6) if cap_cols else (None, 0)
    # No colour assertion on the hint's own text. It is the one band on this page
    # whose ink is *not* the brightest thing in it -- the cap carries a brighter
    # glyph of its own -- so the brightest-ink rule reads the cap's letter and
    # answers `#a2acb6`, and tightening the rule to reach the sentence would be
    # tuning a threshold to a coincidence. The ring and the geometry are asserted;
    # the row's colour is recorded in `REFERENCE.md` instead.
    g.check(
        cap_cols
        and abs(cap_w - HOME["cap_box"]) <= 3
        and abs((h1 - h0 + 1) - HOME["hint_rows"]) <= 2
        and ring is not None
        and near(ring, HOME["base_ring"], 6),
        "[hint] the hint row is 20px with a 20px key cap in the reference's colours",
        f"row {h1 - h0 + 1} rows, cap {cap_w} wide, cap ring "
        + ("#%02x%02x%02x" % ring if ring else "not found"),
    )

    # G26: the block at the bottom -- the prompt, and a 40px button on the basic
    # surface inside a `#42444a` ring, which is what the reference's own Import
    # button measures.
    prompt_ink, _ = core(im, prompt[0], prompt[1] + 1, p0, p1, paper)
    base_fill, _ = modal(im, imp[0] + 8, imp[1] - 8, i0 + 6, i1 - 6, paper, 60)
    ring, _ = modal(im, imp[0], imp[0] + 2, i0 + 10, i1 - 10, paper, 20)
    g.check(
        prompt_ink is not None
        and near(prompt_ink, HOME["hint"], 8)
        and abs((p1 - p0 + 1) - HOME["prompt_ink_rows"]) <= 3
        and base_fill is not None
        and near(base_fill, HOME["base_fill"], 4)
        and ring is not None
        and near(ring, HOME["base_ring"], 6)
        and abs((i1 - i0 + 1) - HOME["button_h"]) <= 2,
        "[import button] 40px of the reference's basic surface inside its ring",
        (
            "prompt #%02x%02x%02x, button #%02x%02x%02x (%d rows) ringed %s, in %d "
            "rows against #%02x%02x%02x / #%02x%02x%02x / #%02x%02x%02x"
            % (
                (prompt_ink or (0, 0, 0))
                + (base_fill or (0, 0, 0))
                + (i1 - i0 + 1,)
                + ("#%02x%02x%02x" % ring if ring else "nothing", i1 - i0 + 1)
                + HOME["hint"] + HOME["base_fill"] + HOME["base_ring"]
            )
        ),
    )

    # G27: the gaps, ink row to ink row. These are the page's rhythm, and the
    # reason they are stated as ink rather than as boxes: the two clients' text
    # boxes are different heights (a browser's line box carries half-leading, this
    # toolkit's does not), so a port that copies `gap-6` literally puts its ink in
    # the wrong place while looking right in the stylesheet.
    gaps = (t0 - art_bottom, d0 - t1, b0 - d1, h0 - b1, i0 - p1)
    wanted = (
        HOME["art_to_title"], HOME["title_to_desc"], HOME["desc_to_button"],
        HOME["button_to_hint"], HOME["prompt_to_import"],
    )
    print("gaps: " + ", ".join(f"{a} vs {b}" for a, b in zip(gaps, wanted)))
    g.check(
        all(abs(a - b) <= 6 for a, b in zip(gaps, wanted)),
        "[gaps] the hero's own rhythm, ink to ink",
        f"{gaps} against {wanted}",
    )

    # G28: every band centred on the content's centre -- the column's middle minus
    # the 11px scrollbar gutter the reference's content is centred inside.
    want_x = (pane_left + panel_left - REF["gutter"]) / 2
    centres = {
        "illustration": (art[0] + art[1]) / 2,
        "title": (title[0] + title[1]) / 2,
        "description": (desc[0] + desc[1]) / 2,
        "brand": (brand[0] + brand[1]) / 2,
        "hint": (hint[0] + hint[1]) / 2,
        "import": (imp[0] + imp[1]) / 2,
    }
    off = {k: round(v - want_x, 1) for k, v in centres.items()}
    print(f"centres against {want_x:.1f}: {off}")
    g.check(
        all(abs(v) <= 6 for v in off.values()),
        "[centring] every band is centred where the reference centres it",
        f"offsets {off} against 0",
    )

    # G29: the hero is centred in the space *above* the block at the bottom, which
    # is 18px above the column's middle in both clients.
    hero_centre = (art0 + h1) / 2
    column_middle = (bottom + (top - 20)) / 2
    print(f"hero centre {hero_centre:.1f}, column middle {column_middle:.1f}")
    g.check(
        abs((column_middle - hero_centre) - HOME["hero_above_middle"]) <= 5,
        "[centring] the hero is centred above the block at the bottom",
        f"{column_middle - hero_centre:.1f}px above the middle against "
        f"{HOME['hero_above_middle']}",
    )

    print()
    if not g.ran:
        print(f"no assertion matched --only {only}")
        return 2
    if g.failures:
        print(f"page gate FAILED{scope} ({len(g.failures)}): " + ", ".join(g.failures))
        return 1
    print(f"page gate passed{scope}")
    return 0


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
    # because a page is mostly empty. Named `paper` rather than `page`, which is
    # the name of the page being judged -- the two words are one meaning apart and
    # letting them share a variable silently sent every Home capture down the
    # Screenshots path.
    paper, paper_n = modal(im, x0, x1, 0, h - 1, (0, 0, 0), 0)
    print(
        f"column x {pane_left}..{panel_left} (scores {pane_score}/{panel_score}), "
        f"page #{paper[0]:02x}{paper[1]:02x}{paper[2]:02x} over {paper_n} px"
    )
    g.check(
        near(paper, REF["page"], 3),
        "[page] the column is the reference's page colour",
        "#%02x%02x%02x against #%02x%02x%02x" % (paper + REF["page"]),
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
    bottom = page_bottom(im, x0, x1, top, paper)
    print(
        f"page top y={bar_bottom} (score {bar_score}), scanning from y={top} "
        f"to y={bottom} where the page colour ends"
    )
    # Home judges itself, from here: the cluster of a welcome screen is not the
    # cluster of an empty state, and the numbers are its own (`HOME` above).
    if page == "home":
        return home_gate(g, im, x0, x1, paper, pane_left, panel_left, top, bottom, scope, only)

    found = bands(rows_with_content(im, x0, x1, top, bottom, paper, SHAPE))
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

    art = columns_in(im, x0, x1, art_y0, art_y1, paper, SHAPE)
    head = columns_in(im, x0, x1, head_y0, head_y1, paper, INK)
    sub = columns_in(im, x0, x1, sub_y0, sub_y1, paper, INK)
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
    fill, fill_n = modal(im, art[0], art[1] + 1, art_y0, art_y1, paper, 2)
    outline, outline_n = modal(im, art[0], art[1] + 1, art_y0, art_y1, paper, 60)
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
    head_ink_color, _ = modal(im, head[0], head[1] + 1, head_y0, head_y1, paper, INK)
    g.check(
        head_ink_color is not None
        and near(head_ink_color, REF["heading"], 6)
        and abs(head_ink - REF["heading_ink_rows"]) <= 2,
        "[heading] the heading is the reference's 24px bold white",
        "#%02x%02x%02x in %d ink rows against #%02x%02x%02x in %d"
        % (head_ink_color + (head_ink,) + REF["heading"] + (REF["heading_ink_rows"],)),
    )
    sub_ink_color, _ = modal(im, sub[0], sub[1] + 1, sub_y0, sub_y1, paper, INK)
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
