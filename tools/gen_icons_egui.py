#!/usr/bin/env python3
"""Compile the reference client's icon set into geometry an egui window can paint.

`tools/gen_icons.py` does the reading half of this job already: it walks
`vendor/modrinth-app/assets/icons`, parses the SVG subset these icons use -- paths,
strokes, fills, circles/rects as paths, view boxes, group transforms -- and hands
back a view box plus a list of `Element`s in absolute `Move`/`Line`/`Cubic`/`Close`
commands. None of that changes for egui, and a second SVG reader written beside the
first is exactly the thing that drifts from it, so this tool **imports** that one:

    import gen_icons                      # tools/gen_icons.py
    box, elements = gen_icons.read_icon(...)

What does change is the shape vocabulary at the far end. iced's `Path::builder`
takes a sequence of subpaths and cubic Beziers; egui's `epaint::PathShape` takes a
`Vec<Pos2>` -- one polyline, no curves, no subpaths -- plus a fill and a
`PathStroke`. So this tool does the two conversions egui needs and iced did not:

* **Flatten the cubics.** Each Bezier is subdivided until its control points are
  within `FLATNESS_FRACTION` of the view box's long side from its chord. At the
  sizes a launcher draws (16--48 px) that is under a tenth of a pixel, and the
  tolerance is measured against the view box rather than fixed in units, so the
  512-unit tag icons and the 14x2 one get the same visual accuracy.
* **Split the subpaths.** A `PathShape` is a single polyline, so one SVG element
  with three subpaths becomes three shapes. This is exact for strokes -- stroking
  a subpath is independent of its siblings -- and it is the reason a filled element
  with holes cannot be expressed here at all: see `--report` and the module
  docstring of the generated file.

    python tools/gen_icons_egui.py            # write crates/palantir-egui/src/icons.rs
    python tools/gen_icons_egui.py --check    # fail if the file on disk is not this
    python tools/gen_icons_egui.py --report   # glyph/shape counts and what cannot be drawn

The glyph list, its order and its names are the iced generator's, not a second
opinion about them: both iterate `gen_icons.SOURCES` in the same order with the
same `variant()` naming, so `icons.rs` and `icons_gen.rs` can be diffed a glyph at
a time, and `the_enum_matches_the_iced_generator_variant_for_variant` in the
generated test module is what keeps that true.

Stdlib only, like the generator it borrows from.
"""

from __future__ import annotations

import argparse
import math
import sys
from pathlib import Path

# The parser lives in the sibling tool; put its directory on the path rather than
# copying a function out of it. `gen_icons` has no import-time side effects (its
# `main()` is behind `if __name__`), so importing it only defines things.
TOOLS = Path(__file__).resolve().parent
if str(TOOLS) not in sys.path:
    sys.path.insert(0, str(TOOLS))

import gen_icons  # noqa: E402  -- after the path fix-up above, and it is the point

ROOT = gen_icons.ROOT
OUT = ROOT / "crates" / "palantir-egui" / "src" / "icons.rs"

# How flat a flattened cubic has to be, as a fraction of the view box's long side.
#
# In device pixels that is the same fraction of the drawn size whatever the icon's
# own units are: 0.048 units for a 24-unit glyph, which is 0.05 px at 24 px and
# 0.1 px at 48 px, against epaint's half-pixel feathering. Smaller is invisible and
# costs points in a several-hundred-kB file; larger starts to show on the 48 px
# icons.
FLATNESS_FRACTION = 0.002

# Subdivision depth cap. The flatness test stops long before this on every curve in
# the set; the cap is here so a pathological control polygon cannot recurse forever.
MAX_SUBDIVISION = 16

# Decimals kept on a coordinate. A view-box unit is at most one device pixel for
# the 24-unit icons, so three decimals is a thousandth of a pixel; it also keeps
# the emitted file from carrying the full binary expansion of every intersection.
DECIMALS = 3

# Decimals kept on a stroke width, matching `tools/gen_icons.py`'s own `fnum`, so a
# reader can compare the two files' widths digit for digit.
WIDTH_DECIMALS = 4


# --------------------------------------------------------------------------
# Flattening
# --------------------------------------------------------------------------


def midpoint(a: tuple[float, float], b: tuple[float, float]) -> tuple[float, float]:
    return ((a[0] + b[0]) * 0.5, (a[1] + b[1]) * 0.5)


def split_cubic(p0, p1, p2, p3):
    """De Casteljau at t = 0.5: the two cubics that are the same curve."""
    a, b, c = midpoint(p0, p1), midpoint(p1, p2), midpoint(p2, p3)
    d, e = midpoint(a, b), midpoint(b, c)
    f = midpoint(d, e)
    return (p0, a, d, f), (f, e, c, p3)


def point_to_segment(point, start, end) -> float:
    """Perpendicular distance from `point` to the line `start`--`end`.

    `start == end` (a curve that returns to where it began) has no line, so the
    distance to the point is used instead -- otherwise every closed loop would read
    as flat and be emitted as a single zero-length segment.
    """
    dx, dy = end[0] - start[0], end[1] - start[1]
    length = math.hypot(dx, dy)
    if length == 0.0:
        return math.hypot(point[0] - start[0], point[1] - start[1])
    return abs(dx * (point[1] - start[1]) - dy * (point[0] - start[0])) / length


def cubic_is_flat(p0, p1, p2, p3, tolerance: float) -> bool:
    return (point_to_segment(p1, p0, p3) <= tolerance
            and point_to_segment(p2, p0, p3) <= tolerance)


def flatten_cubic(out: list[tuple[float, float]], p0, p1, p2, p3,
                  tolerance: float, depth: int = 0) -> None:
    """Append the points of one cubic to `out`, excluding `p0`.

    Recursive halving rather than a fixed segment count: a curve that is nearly
    straight (most of the icon set's corners) costs one segment, and the circle in
    `circle.svg` costs as many as it needs.
    """
    if depth >= MAX_SUBDIVISION or cubic_is_flat(p0, p1, p2, p3, tolerance):
        out.append(p3)
        return
    left, right = split_cubic(p0, p1, p2, p3)
    flatten_cubic(out, *left, tolerance, depth + 1)
    flatten_cubic(out, *right, tolerance, depth + 1)


def rounded(value: float) -> float:
    """`value` at the file's precision, with `-0.0` folded onto `0.0`."""
    result = round(value, DECIMALS)
    return 0.0 if result == 0.0 else result


def tidy(points: list[tuple[float, float]], closed: bool) -> list[tuple[float, float]]:
    """Round, drop repeated neighbours, and drop the closing point if repeated.

    A `PathShape` closes a loop itself, so a final point equal to the first one is a
    zero-length segment. epaint survives one -- its normal comes out as the zero
    vector -- but a zero-length segment is still a joint the stroker has to join,
    and there is no reason to hand it one.
    """
    out: list[tuple[float, float]] = []
    for point in points:
        candidate = (rounded(point[0]), rounded(point[1]))
        if out and out[-1] == candidate:
            continue
        out.append(candidate)
    if closed and len(out) > 1 and out[0] == out[-1]:
        out.pop()
    return out


def subpaths(element, tolerance: float) -> list[tuple[list[tuple[float, float]], bool]]:
    """One element's geometry as the polylines egui can express.

    An element the reference paints nothing for (`Cmd::NoPaint`, the clamp path an
    exported icon carries) has no shape at all and comes back empty: SVG draws
    nothing for it, and so does this.

    `closed` is true for a subpath that ends in `Close` *and* for every filled one,
    because a fill closes its contour whether the SVG says so or not.
    """
    if element.commands and element.commands[0][0] == "NoPaint":
        return []

    out: list[tuple[list[tuple[float, float]], bool]] = []
    points: list[tuple[float, float]] = []
    current = (0.0, 0.0)
    closed = False

    def flush() -> None:
        nonlocal points, closed
        cleaned = tidy(points, closed or element.filled)
        if len(cleaned) >= (3 if element.filled else 2):
            out.append((cleaned, closed or element.filled))
        points, closed = [], False

    for command in element.commands:
        kind = command[0]
        if kind == "Move":
            if points:
                flush()
            current = (command[1], command[2])
            points = [current]
        elif kind == "Line":
            current = (command[1], command[2])
            points.append(current)
        elif kind == "Cubic":
            control1 = (command[1], command[2])
            control2 = (command[3], command[4])
            end = (command[5], command[6])
            flatten_cubic(points, current, control1, control2, end, tolerance)
            current = end
        elif kind == "Close":
            closed = True
    if points:
        flush()
    return out


# --------------------------------------------------------------------------
# What epaint's fill cannot express
# --------------------------------------------------------------------------


def cross(o, a, b) -> float:
    return (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0])


def is_convex(points: list[tuple[float, float]]) -> bool:
    """Whether the points, in order, wind the same way at every corner.

    The tolerance is relative to the edge lengths so that a nearly-straight corner
    after rounding does not read as a reversal: the emitted coordinates are only
    kept to three decimals.
    """
    count = len(points)
    if count < 3:
        return False
    sign = 0
    for index in range(count):
        o, a, b = points[index], points[(index + 1) % count], points[(index + 2) % count]
        value = cross(o, a, b)
        scale = math.hypot(a[0] - o[0], a[1] - o[1]) * math.hypot(b[0] - a[0], b[1] - a[1])
        if scale == 0.0 or abs(value) <= 1e-9 * scale:
            continue
        corner = 1 if value > 0 else -1
        if sign == 0:
            sign = corner
        elif sign != corner:
            return False
    return True


def point_inside(point, polygon: list[tuple[float, float]]) -> bool:
    """Ray casting: is `point` inside `polygon`? Boundary cases are not relied on."""
    inside = False
    count = len(polygon)
    for index in range(count):
        x1, y1 = polygon[index]
        x2, y2 = polygon[(index + 1) % count]
        if (y1 > point[1]) != (y2 > point[1]):
            crossing = x1 + (point[1] - y1) * (x2 - x1) / (y2 - y1)
            if crossing > point[0]:
                inside = not inside
    return inside


def fill_limitation(shape_list: list[tuple[list[tuple[float, float]], bool]]) -> str | None:
    """Why epaint cannot draw this filled element, or `None` if it can.

    `PathShape` has a fill colour and no fill rule, and epaint's fill is a triangle
    fan from the first point of the loop, documented as being for convex polygons.
    So a contour that is one convex loop is exact; disjoint convex loops are exact
    one shape each; anything with a hole or a concave outline is not.
    """
    for points, _ in shape_list:
        if not is_convex(points):
            return "concave outline"
    if len(shape_list) > 1:
        for outer_index, (outer, _) in enumerate(shape_list):
            for inner_index, (inner, _) in enumerate(shape_list):
                if outer_index == inner_index:
                    continue
                if any(point_inside(point, outer) for point in inner):
                    return "hole (a subpath nested inside another)"
    return None


# --------------------------------------------------------------------------
# Reading the reference
# --------------------------------------------------------------------------


def collect() -> tuple[list[tuple], dict[str, list[tuple]], list[tuple[str, str]]]:
    """Every icon the iced generator emits, in its order, with its geometry.

    This is the collection half of `gen_icons.main()` -- the same directory walk,
    the same prefixing, the same duplicate-name guard, the same refusal policy --
    repeated rather than called because `main()` also writes the iced file and
    returns an exit code. The *parsing* is not repeated: `read_icon` is imported.
    """
    icons: list[tuple] = []
    sets: dict[str, list[tuple]] = {}
    refused: list[tuple[str, str]] = []

    for source in gen_icons.SOURCES:
        directory = gen_icons.ICONS / source["directory"] if source["directory"] else gen_icons.ICONS
        files = sorted(directory.glob(source["glob"]))
        if not files:
            raise SystemExit(f"no icons in {directory}")
        collected: list[tuple] = []
        for path in files:
            name = (Path(source["directory"]) / path.stem).as_posix() if source["directory"] \
                else path.stem
            try:
                box, elements = gen_icons.read_icon(path, name, [], [], [], [])
            except gen_icons.Unsupported as exc:
                if source["complete"] or name not in gen_icons.REFUSED_TAGS:
                    raise SystemExit(f"{name}.svg: {exc}")
                refused.append((name, str(exc)))
                continue
            collected.append((name, source["prefix"] + gen_icons.variant(path.stem), box, elements))
        sets[source["table"]] = collected
        icons.extend(collected)

    seen: dict[str, str] = {}
    for name, rust, _, _ in icons:
        if rust in seen:
            raise SystemExit(f"{seen[rust]} and {name} both become `{rust}`")
        seen[rust] = name
    return icons, sets, refused


# --------------------------------------------------------------------------
# Emitting Rust
# --------------------------------------------------------------------------


def number(value: float, places: int = DECIMALS) -> str:
    """A float the way Rust spells it: always with a decimal point."""
    if value == 0.0:
        value = 0.0  # folds -0.0 onto 0.0
    text = f"{value:.{places}f}".rstrip("0").rstrip(".")
    if text in ("", "-"):
        text = "0"
    return text if "." in text else text + ".0"


def point_source(point: tuple[float, float]) -> str:
    return f"({number(point[0])}, {number(point[1])})"


def points_source(points: list[tuple[float, float]]) -> list[str]:
    """The `&[...]` literal for one polyline, four points to a line."""
    per_line = 4
    out = ["        points: &["]
    for start in range(0, len(points), per_line):
        chunk = ", ".join(point_source(point) for point in points[start:start + per_line])
        out.append(f"            {chunk},")
    out.append("        ],")
    return out


def ink_source(ink: tuple[str, str | None]) -> str:
    kind, value = ink
    if kind == "Fixed" and value is not None:
        red, green, blue, alpha = value.split(",")
        return f"Ink::Fixed({red}, {green}, {blue}, {alpha})"
    if kind == "Token" and value is not None:
        return f'Ink::Token("{value}")'
    return "Ink::Inherit"


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--check", action="store_true",
                        help="fail if the checked-in file is not what this would emit")
    parser.add_argument("--report", action="store_true", help="print per-icon detail")
    args = parser.parse_args()

    if not gen_icons.ICONS.is_dir():
        print(f"missing reference icons: {gen_icons.ICONS}", file=sys.stderr)
        return 2

    icons, sets, refused = collect()

    # Everything the file will hold, worked out once: the glyph's shapes, how many
    # of them, and which filled elements epaint cannot fill.
    geometry: list[tuple] = []
    not_expressible: list[tuple[str, int, str, bool]] = []
    no_paint: list[str] = []
    filled_elements = 0
    for name, rust, box, elements in icons:
        tolerance = max(FLATNESS_FRACTION * max(box), 1e-4)
        shapes: list[tuple[list[tuple[float, float]], bool, object]] = []
        for index, element in enumerate(elements):
            if element.commands and element.commands[0][0] == "NoPaint":
                no_paint.append(f"{name}#{index}")
                continue
            found = subpaths(element, tolerance)
            if element.filled:
                filled_elements += 1
                reason = fill_limitation(found)
                if reason is not None:
                    not_expressible.append((name, index, reason, element.even_odd))
            for points, closed in found:
                shapes.append((points, closed, element))
        if not shapes:
            raise SystemExit(f"{name}.svg: no shape egui could draw")
        geometry.append((name, rust, box, elements, shapes))

    shape_counts: list[tuple[str, str, int]] = [
        (name, rust, len(shapes)) for name, rust, _, _, shapes in geometry
    ]
    total_shapes = sum(count for _, _, count in shape_counts)
    total_points = sum(len(points) for _, _, _, _, shapes in geometry for points, _, _ in shapes)
    largest = max(shape_counts, key=lambda entry: entry[2])
    width_exceptions = sorted({
        f"{name}:{number(element.stroke_width, WIDTH_DECIMALS)}"
        for name, _, _, elements, _ in geometry
        for element in elements
        if abs(element.stroke_width - 2.0) > 0.01
    })
    # The tokens a shape is painted with, as `glyph:--token`, sorted and unique.
    tokens = sorted({
        f"{name}:{element.ink[1]}"
        for name, _, _, elements, _ in geometry
        for element in elements
        if element.ink[0] == "Token"
    })
    fixed_colours = sorted({
        name for name, _, _, elements, _ in geometry
        for element in elements if element.ink[0] == "Fixed"
    })
    filled_glyphs = sorted({name for name, _, _, elements, _ in geometry
                            if any(element.filled for element in elements)})
    even_odds = sorted({
        f"{name}#{index}"
        for name, _, _, elements, _ in geometry
        for index, element in enumerate(elements)
        if element.filled and element.even_odd
    })
    unsupported_by_glyph: dict[str, list[str]] = {}
    for name, index, reason, even_odd in not_expressible:
        rule = ', `fill-rule="evenodd"`' if even_odd else ""
        unsupported_by_glyph.setdefault(name, []).append(f"element {index + 1}: {reason}{rule}")

    numbers: dict[str, object] = {
        "glyphs": len(icons),
        "all": len(sets["ALL"]),
        "categories": len(sets["TAG_CATEGORIES"]),
        "loaders": len(sets["TAG_LOADERS"]),
        "shapes": total_shapes,
        "points": total_points,
        "elements": sum(len(elements) for _, _, _, elements, _ in geometry),
        "no_paint": len(no_paint),
        "filled_elements": filled_elements,
        "filled_glyphs": len(filled_glyphs),
        "unsupported": len(not_expressible),
        "flatness": FLATNESS_FRACTION,
    }

    lines: list[str] = []
    add = lines.append

    add("//! The reference client's icon set, compiled into egui shapes.")
    add("//!")
    add("//! **Generated by `tools/gen_icons_egui.py`. Do not edit.**")
    add("//!")
    add("//! This is the egui twin of `crates/palantir-desktop/src/icons_gen.rs`: the")
    add(f"//! same {numbers['glyphs']} SVGs out of `vendor/modrinth-app/assets/icons`, read by")
    add("//! the same parser, listed in the same order under the same names, so the two")
    add("//! files can be diffed a glyph at a time. The parser is not copied --")
    add("//! `tools/gen_icons_egui.py` imports `tools/gen_icons.py` and calls its")
    add("//! `read_icon` -- because two SVG readers in one tree is two answers to what")
    add("//! an icon looks like, and only one of them gets fixed.")
    add("//!")
    add("//! | table | where | variants | glyphs |")
    add("//! | --- | --- | --- | --- |")
    for source in gen_icons.SOURCES:
        where = f"`icons/{source['directory']}`" if source["directory"] else "`icons/`"
        add(f"//! | `{source['table']}` | {where} | `{source['prefix'] or 'none'}` | "
            f"{len(sets[source['table']])} |")
    add("//!")
    add("//! The prefixes are the reference's own, and the four tag icons the parser")
    add("//! refuses are absent here for the same reason they are absent there:")
    add("//!")
    for name in sorted(gen_icons.REFUSED_TAGS):
        add(f"//! * `{name}.svg` -- {gen_icons.REFUSED_TAGS[name]}")
    add("//!")
    add("//! ## What is different from the iced file")
    add("//!")
    add("//! **A `PathShape` is one polyline.** iced's path builder holds subpaths and")
    add("//! cubic Beziers; `epaint::PathShape` holds a `Vec<Pos2>` and nothing else. So")
    add("//! the generator flattens every cubic at generation time -- recursive halving")
    add("//! until both control points are within {flatness} of the view box's long".format(**numbers))
    add("//! side of the chord, which is 0.05 px at 24 px and 0.1 px at 48 px, well")
    add("//! under epaint's half-pixel feathering -- and splits every element into one")
    add("//! shape per subpath, because a `PathShape` cannot hold two. Flattening here")
    add("//! rather than per frame is the whole reason the coordinates in this file are")
    add(f"//! plain numbers: {numbers['points']} of them, and not one is computed twice.")
    add("//!")
    add("//! Splitting subpaths is exact for strokes: stroking a subpath does not")
    add("//! depend on its siblings, which is why the iced side's multi-subpath paths")
    add("//! and these shapes draw the same picture. It is **not** exact for fills, and")
    add("//! that is the one thing this file cannot say:")
    add("//!")
    add(f"//! **{numbers['unsupported']} of the {numbers['filled_elements']} filled elements have no")
    add("//! egui shape.** `PathShape` has a fill colour and no fill rule, and epaint")
    add("//! fills a path with a triangle fan from its first point, documented as")
    add("//! convex-only. An element that is one convex contour is therefore exact, and")
    add("//! so is one made of disjoint convex contours -- each becomes its own shape.")
    add("//! An element with a **hole** or a **concave outline** is not: the hole is")
    add("//! filled solid, or the fan covers ground the outline does not. These:")
    add("//!")
    for name in sorted(unsupported_by_glyph):
        add(f"//! * `{name}.svg` -- {'; '.join(unsupported_by_glyph[name])}")
    add("//!")
    add("//! The iced side carries the reference's `fill-rule` and honours it, so those")
    add("//! glyphs are drawn correctly there and only approximately here. Nothing else")
    add(f"//! in the set is affected: the other {numbers['filled_elements'] - numbers['unsupported']}")
    add("//! filled elements are expressible.")
    add("//!")
    add(f"//! **{numbers['no_paint']} elements are not here at all.** They are the shapes the")
    add("//! reference declares with neither a fill nor a stroke -- the clamp path every")
    add("//! exported icon carries, `M0 0h24v24H0Z` -- which SVG paints nothing for. The")
    add("//! iced file emits them as `Cmd::NoPaint` so a reader can see it read them; a")
    add("//! shape that paints nothing has no `PathShape` to be, so they are dropped and")
    add("//! named here instead:")
    add("//!")
    for entry in sorted(no_paint):
        add(f"//! * `{entry.split('#')[0]}.svg` element {int(entry.split('#')[1]) + 1}")
    add("//!")
    add("//! **The pen is not the same pen.** `PathStroke` carries a width, a colour and")
    add("//! a `StrokeKind`, and no line cap or join; epaint's stroker joins with miters")
    add("//! and has no round caps. The reference declares")
    add("//! `stroke-linecap=\"round\"`/`stroke-linejoin=\"round\"` on 413 of the 441")
    add("//! glyphs, so a corner here is slightly sharper than the reference's and a")
    add("//! line end is square rather than round. That is a property of the toolkit,")
    add("//! not of this file's data, and there is no attribute to set to change it.")
    add("//!")
    add("//! ## Performance")
    add("//!")
    add("//! These shapes are drawn every frame a window draws, so the cost model matters")
    add("//! more than the last decimal of a coordinate.")
    add("//!")
    add("//! * **The geometry is `&'static`.** No SVG is parsed at runtime, no path is")
    add("//!   built from a string, and the coordinates live in the binary. Drawing a")
    add("//!   glyph allocates one `Vec<Shape>` and one `Vec<Pos2>` per path, both")
    add("//!   pre-sized with `Vec::with_capacity`, and nothing else.")
    add("//! * **One paint-list entry per glyph, not one per path.** `draw` and `cached`")
    add("//!   hand egui a single `Shape::Vec`, so a 15-shape glyph costs one entry in")
    add("//!   the layer's paint list and one clip decision rather than fifteen, and the")
    add("//!   tessellator walks a contiguous batch.")
    add("//! * **What is *not* saved: tessellation.** egui caches tessellation for one")
    add("//!   frame and then throws it away -- there is no cross-frame mesh cache in the")
    add("//!   toolkit -- so a glyph that moves is tessellated again next frame. The cost")
    add("//!   this module removes is the allocation, the path building and the")
    add("//!   paint-list churn, not the triangles. `cached` goes one step further and")
    add("//!   keeps the built shapes across frames for a glyph that has not changed:")
    add("//!   a hit is a `Vec` clone, which is a memcpy of the points that were going to")
    add("//!   be computed anyway, and a miss is the same build `draw` does.")
    add("//!")
    add("//! A baked `Shape::Mesh` is the other route, and it is deliberately not taken.")
    add("//! A mesh is a finished triangle list, so adding one skips tessellation")
    add("//! entirely -- but egui bakes *absolute* positions into it and has no")
    add("//! per-shape transform at paint time, so a mesh cache has to include the")
    add("//! widget's position in its key. A scrolling list moves its icons every frame;")
    add("//! every one of them would miss, re-tessellate through a private `Tessellator`")
    add("//! per icon per frame, and the bounded cache would clear itself continuously.")
    add("//! That is strictly worse than handing shapes to the single tessellation pass")
    add("//! egui already runs at the end of the frame. Where icons do not move, the")
    add("//! shape route's cache hit is already a memcpy.")
    add("")
    add("// Nothing draws these yet: `crates/palantir-egui` is the first slice of the")
    add("// move from iced to egui (see its `Cargo.toml`), and it is a frame-time")
    add("// harness rather than the launcher. Delete this line the day the last icon is")
    add("// drawn from here.")
    add("#![allow(dead_code)]")
    add("")
    add("use std::cell::RefCell;")
    add("")
    add("use egui::epaint::{PathShape, PathStroke};")
    add("use egui::layers::ShapeIdx;")
    add("use egui::{Color32, Painter, Pos2, Rect, Shape, Vec2};")
    add("")
    add("/// Where a subpath's colour comes from.")
    add("///")
    add("/// The same three answers the iced file gives, because they are properties of")
    add("/// the reference's SVGs rather than of the toolkit: `currentColor` is what")
    add("/// almost every shape in the set asks for, a literal is a shape painted red or")
    add("/// white on purpose, and a `var(--token)` is one the shell resolves.")
    add("#[derive(Clone, Copy, Debug, PartialEq, Eq)]")
    add("pub enum Ink {")
    add("    /// `currentColor`: the colour of whatever the icon is drawn in.")
    add("    Inherit,")
    add("    /// A literal the reference wrote out.")
    add("    Fixed(u8, u8, u8, u8),")
    add("    /// A `var(--token)` the reference names.")
    add("    Token(&'static str),")
    add("}")
    add("")
    add("impl Ink {")
    add("    /// The colour this shape is, given what the caller is drawing with.")
    add("    pub fn resolve(self, inherit: Color32) -> Color32 {")
    add("        match self {")
    add("            Ink::Inherit => inherit,")
    add("            Ink::Fixed(r, g, b, a) => Color32::from_rgba_unmultiplied(r, g, b, a),")
    add("            // A token the shell has not been taught yet is drawn in the ink it")
    add("            // would have had anyway, which is visible rather than invisible.")
    add("            Ink::Token(_) => inherit,")
    add("        }")
    add("    }")
    add("}")
    add("")
    add("/// One drawable piece of an icon: a single polyline and how to paint it.")
    add("///")
    add("/// This is one *subpath*, not one SVG element. An element with three subpaths")
    add("/// is three of these, because a `PathShape` cannot hold two -- see the module")
    add("/// documentation. The iced file's `Element` is the element-level record; this")
    add("/// is the shape-level one.")
    add("#[derive(Clone, Copy, Debug, PartialEq)]")
    add("pub struct Subpath {")
    add("    /// The polyline, in the icon's own view box, as x/y pairs.")
    add("    pub points: &'static [(f32, f32)],")
    add("    /// Whether it closes back to its first point. True for every filled one.")
    add("    pub closed: bool,")
    add("    /// Stroke width in view-box units, before the fit scale is applied.")
    add("    pub stroke_width: f32,")
    add("    /// Whether this is filled rather than stroked.")
    add("    pub filled: bool,")
    add("    /// Where its colour comes from.")
    add("    pub ink: Ink,")
    add("    /// The element's own `opacity`, which multiplies whatever ink it gets.")
    add("    pub opacity: f32,")
    add("}")
    add("")
    add("/// Every icon in the reference's three sets, in the iced file's order.")
    add("#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]")
    add("pub enum Glyph {")
    for name, rust, _, _, _ in geometry:
        add(f"    /// `{name}.svg`")
        add(f"    {rust},")
    add("}")
    add("")
    add("/// The top-level set, in name order, with the file name beside each.")
    add("pub const ALL: &[(&str, Glyph)] = &[")
    for name, rust, _, _ in sets["ALL"]:
        add(f'    ("{name}", Glyph::{rust}),')
    add("];")
    add("")
    add("/// `icons/tags/categories/*.svg`, in name order: a category tag's own icon.")
    add("pub const TAG_CATEGORIES: &[(&str, Glyph)] = &[")
    for name, rust, _, _ in sets["TAG_CATEGORIES"]:
        add(f'    ("{name}", Glyph::{rust}),')
    add("];")
    add("")
    add("/// `icons/tags/loaders/*.svg`, in name order: a loader tag's own icon.")
    add("pub const TAG_LOADERS: &[(&str, Glyph)] = &[")
    for name, rust, _, _ in sets["TAG_LOADERS"]:
        add(f'    ("{name}", Glyph::{rust}),')
    add("];")
    add("")
    add("/// The three tables, in the order they are declared above.")
    add("fn tables() -> [&'static [(&'static str, Glyph)]; 3] {")
    add(f"    [{', '.join(source['table'] for source in gen_icons.SOURCES)}]")
    add("}")
    add("")
    add("impl Glyph {")
    add("    /// The icon's own file name below `assets/icons`, without the extension.")
    add("    pub fn name(self) -> &'static str {")
    add("        tables()")
    add("            .iter()")
    add("            .flat_map(|table| table.iter())")
    add("            .find(|(_, glyph)| *glyph == self)")
    add("            .map(|(name, _)| *name)")
    add("            .unwrap_or(\"\")")
    add("    }")
    add("")
    add("    /// The icon a name refers to, for a gate that walks the reference.")
    add("    pub fn by_name(name: &str) -> Option<Glyph> {")
    add("        tables()")
    add("            .iter()")
    add("            .flat_map(|table| table.iter())")
    add("            .find(|(candidate, _)| *candidate == name)")
    add("            .map(|(_, glyph)| *glyph)")
    add("    }")
    add("")
    add("    /// The view box the points are expressed in, as `(width, height)`.")
    add("    pub fn view_box(self) -> (f32, f32) {")
    add("        match self {")
    for name, rust, box, _, _ in geometry:
        add(f"            Glyph::{rust} => ({number(box[0])}, {number(box[1])}),")
    add("        }")
    add("    }")
    add("")
    add("    /// This icon's shapes, in the reference's document order.")
    add("    pub fn subpaths(self) -> &'static [Subpath] {")
    add("        match self {")
    for name, rust, _, _, _ in geometry:
        add(f"            Glyph::{rust} => &PATHS_{gen_icons.screaming(rust)},")
    add("        }")
    add("    }")
    add("}")
    add("")
    add("/// Where a glyph's view box lands inside a rectangle: a uniform scale and the")
    add("/// offset its top-left corner takes.")
    add("///")
    add("/// The iced side returns `(scale, Vector)` from `icon::fit`; this is that pair")
    add("/// as one value, so a caller cannot pass the scale without the offset.")
    add("#[derive(Clone, Copy, Debug, PartialEq)]")
    add("pub struct Fit {")
    add("    /// View-box units to points.")
    add("    pub scale: f32,")
    add("    /// Where the view box's top-left corner lands, relative to the origin.")
    add("    pub offset: Vec2,")
    add("}")
    add("")
    add("/// How the spare space is split around the glyph: half each side, `xMidYMid`.")
    add("///")
    add("/// This is the constant `icon::fit` on the iced side also uses -- its offset is")
    add("/// `(size - drawn) * 0.5` -- and it is the SVG default, not a choice this port")
    add("/// made: none of the 441 SVGs declares `preserveAspectRatio`, so all of them")
    add("/// take `xMidYMid meet`. `meet` is the `min` of the two axis scales in [`fit`];")
    add("/// `xMid`/`yMid` are this 0.5.")
    add("pub const FIT_CENTRE: f32 = 0.5;")
    add("")
    add("/// Where a glyph's view box lands inside `rect`: uniform scale, centred.")
    add("///")
    add("/// The scale is `min(rect.width / view_box.width, rect.height / view_box.height)`")
    add("/// -- `meet`, so the whole icon is visible -- and the glyph is centred in the")
    add("/// rectangle. For the square rectangles a control actually hands an icon, this")
    add("/// is `icon::fit`'s answer exactly; for a non-square one it centres in the whole")
    add("/// rectangle rather than in the largest square at its top-left, which is what")
    add("/// `fits into rect` has to mean.")
    add("///")
    add("/// A rectangle with no room in it -- a widget the layout gave zero pixels, which")
    add("/// happens on the frame a panel collapses -- gives the iced side's degenerate")
    add("/// answer: scale 1, no offset. Drawing at scale 1 into a zero-sized clip is")
    add("/// invisible, and it is not a panic or a division by zero.")
    add("pub fn fit(glyph: Glyph, rect: Rect) -> Fit {")
    add("    let (width, height) = glyph.view_box();")
    add("    let size = rect.width().min(rect.height());")
    add("    if width <= 0.0 || height <= 0.0 || size <= 0.0 {")
    add("        return Fit {")
    add("            scale: 1.0,")
    add("            offset: Vec2::ZERO,")
    add("        };")
    add("    }")
    add("    let scale = (size / width).min(size / height);")
    add("    let drawn = Vec2::new(width * scale, height * scale);")
    add("    Fit {")
    add("        scale,")
    add("        offset: rect.min.to_vec2() + (rect.size() - drawn) * FIT_CENTRE,")
    add("    }")
    add("}")
    add("")
    add("/// The shapes for one glyph, ready for `Painter::add`.")
    add("///")
    add("/// Coordinates are screen points: the view box has been scaled by `fit.scale`")
    add("/// and moved by `fit.offset`. Stroke widths are scaled by the same factor, in")
    add("/// the same place, because epaint strokes the points it is given at the width")
    add("/// it is given -- it does not scale a stroke with anything -- so an icon at")
    add("/// 20 px would otherwise carry a 24 px icon's 2-unit pen and every edge would")
    add("/// be a fifth too heavy.")
    add("///")
    add("/// One `Shape` per [`Subpath`], and no more: the vector is pre-sized, and each")
    add("/// path's points are written straight into a vector sized for them.")
    add("pub fn parts(glyph: Glyph, fit: Fit, ink: Color32) -> Vec<Shape> {")
    add("    let subpaths = glyph.subpaths();")
    add("    let mut shapes = Vec::with_capacity(subpaths.len());")
    add("    for subpath in subpaths {")
    add("        let mut points = Vec::with_capacity(subpath.points.len());")
    add("        points.extend(subpath.points.iter().map(|&(x, y)| {")
    add("            Pos2::new(x.mul_add(fit.scale, fit.offset.x), y.mul_add(fit.scale, fit.offset.y))")
    add("        }));")
    add("        let resolved = subpath.ink.resolve(ink);")
    add("        let color = if subpath.opacity >= 1.0 {")
    add("            resolved")
    add("        } else {")
    add("            resolved.gamma_multiply(subpath.opacity)")
    add("        };")
    add("        let shape = if subpath.filled {")
    add("            // A fill closes itself; the stroke is the empty one, so epaint does")
    add("            // not also outline what it filled.")
    add("            PathShape::convex_polygon(points, color, PathStroke::NONE)")
    add("        } else if subpath.closed {")
    add("            PathShape::closed_line(")
    add("                points,")
    add("                PathStroke::new(subpath.stroke_width * fit.scale, color),")
    add("            )")
    add("        } else {")
    add("            PathShape::line(points, PathStroke::new(subpath.stroke_width * fit.scale, color))")
    add("        };")
    add("        shapes.push(Shape::Path(shape));")
    add("    }")
    add("    shapes")
    add("}")
    add("")
    add("/// One glyph into `rect`, in `ink`, fitted and centred.")
    add("///")
    add("/// This is the plain call: build the shapes, hand them to the painter as one")
    add("/// `Shape::Vec` entry. [`cached`] is the same drawing with the built shapes kept")
    add("/// between frames.")
    add("pub fn draw(painter: &Painter, glyph: Glyph, rect: Rect, ink: Color32) {")
    add("    let shapes = parts(glyph, fit(glyph, rect), ink);")
    add("    painter.add(Shape::Vec(shapes));")
    add("}")
    add("")
    add("/// How many built glyphs the cache keeps before it starts again from empty.")
    add("///")
    add("/// A screen does not hold more than this many distinct icons, and an icon whose")
    add("/// position changes every frame misses anyway, so a larger cache would only")
    add("/// keep shapes nobody is going to ask for. Clearing wholesale rather than")
    add("/// evicting one entry is deliberate: it bounds both the memory and the work,")
    add("/// and the alternative is a clock hand over a vector for no measured gain.")
    add("const CACHE_LIMIT: usize = 256;")
    add("")
    add("/// Everything that can change a drawn glyph, as bits so it can be a hash key.")
    add("///")
    add("/// The position is in here as well as the size, because the shapes are baked in")
    add("/// screen coordinates: an icon that has moved is a different set of points.")
    add("/// `f32` is not `Eq`, so the three floats are compared by their bits -- two")
    add("/// offsets differing in the last bit are two different pictures anyway.")
    add("#[derive(Clone, Copy, PartialEq, Eq, Hash)]")
    add("struct CacheKey {")
    add("    glyph: Glyph,")
    add("    scale: u32,")
    add("    offset: [u32; 2],")
    add("    ink: [u8; 4],")
    add("}")
    add("")
    add("/// One cache entry: the key it was built from, and the shapes it built.")
    add("struct CacheEntry {")
    add("    key: CacheKey,")
    add("    shapes: Vec<Shape>,")
    add("}")
    add("")
    add("thread_local! {")
    add("    /// The recently built glyphs, per thread.")
    add("    ///")
    add("    /// `RefCell` rather than a `Mutex`: an egui context is driven by one thread,")
    add("    /// and a lock around a per-frame read would be a lock taken per icon to")
    add("    /// protect against something the toolkit does not do.")
    add("    static SHAPES: RefCell<Vec<CacheEntry>> = RefCell::new(Vec::new());")
    add("}")
    add("")
    add("/// The key a glyph draws under. Split out so a test can hold it: a cache that")
    add("/// kept serving shapes after the size, the position or the ink changed would be")
    add("/// a stale picture nothing renders wrongly enough to notice.")
    add("fn cache_key(glyph: Glyph, fit: Fit, ink: Color32) -> CacheKey {")
    add("    CacheKey {")
    add("        glyph,")
    add("        scale: fit.scale.to_bits(),")
    add("        offset: [fit.offset.x.to_bits(), fit.offset.y.to_bits()],")
    add("        ink: ink.to_array(),")
    add("    }")
    add("}")
    add("")
    add("/// The shapes for `key`, from the cache or built and then kept.")
    add("fn cached_shapes(key: CacheKey, glyph: Glyph, fit: Fit, ink: Color32) -> Vec<Shape> {")
    add("    SHAPES.with(|cache| {")
    add("        let mut cache = cache.borrow_mut();")
    add("        if let Some(entry) = cache.iter().find(|entry| entry.key == key) {")
    add("            // A hit: clone the built shapes. That is a copy of the points plus")
    add("            // the per-path vector headers, with no arithmetic and no colour")
    add("            // resolution -- cheaper than the build it replaces, and it is what")
    add("            // the painter needs to own its shapes.")
    add("            return entry.shapes.clone();")
    add("        }")
    add("        let shapes = parts(glyph, fit, ink);")
    add("        if cache.len() >= CACHE_LIMIT {")
    add("            cache.clear();")
    add("        }")
    add("        cache.push(CacheEntry {")
    add("            key,")
    add("            shapes: shapes.clone(),")
    add("        });")
    add("        shapes")
    add("    })")
    add("}")
    add("")
    add("/// One glyph into `rect`, in `ink`, with the built shapes kept between frames.")
    add("///")
    add("/// Draw this instead of [`draw`] wherever the icon is not moving: a rail, a")
    add("/// button, a header. The first frame builds and stores; later frames hand egui a")
    add("/// clone of the stored shapes. An icon that *is* moving -- a row scrolling under")
    add("/// the pointer -- misses every frame, because the position is part of the key,")
    add("/// and pays the same build `draw` would have paid. That is not a regression, it")
    add("/// is the reason the cache cannot be the only entry point.")
    add("///")
    add("/// Returns the index the shapes were added at, for a caller that wants to")
    add("/// replace them later with `Painter::set`.")
    add("pub fn cached(painter: &Painter, glyph: Glyph, rect: Rect, ink: Color32) -> ShapeIdx {")
    add("    let fit = fit(glyph, rect);")
    add("    let shapes = cached_shapes(cache_key(glyph, fit, ink), glyph, fit, ink);")
    add("    painter.add(Shape::Vec(shapes))")
    add("}")
    add("")

    for name, rust, box, elements, shapes in geometry:
        add(f"/// `{name}.svg`, {len(elements)} element(s) as {len(shapes)} shape(s).")
        add(f"const PATHS_{gen_icons.screaming(rust)}: [Subpath; {len(shapes)}] = [")
        for points, closed, element in shapes:
            kind = "filled" if element.filled else "stroked"
            add(f"    // {kind} at {number(element.stroke_width, WIDTH_DECIMALS)}, "
                f"{len(points)} point(s)")
            add("    Subpath {")
            lines.extend(points_source(points))
            add(f"        closed: {'true' if closed else 'false'},")
            add(f"        stroke_width: {number(element.stroke_width, WIDTH_DECIMALS)},")
            add(f"        filled: {'true' if element.filled else 'false'},")
            add(f"        ink: {ink_source(element.ink)},")
            add(f"        opacity: {number(element.opacity)},")
            add("    },")
        add("];")
        add("")

    add("#[cfg(test)]")
    add("mod tests {")
    add("    use super::*;")
    add("")
    add("    /// A 24-pixel square, which is the common case: 423 of the 441 glyphs")
    add("    /// declare `viewBox=\"0 0 24 24\"`.")
    add("    fn sample_box() -> Rect {")
    add("        Rect::from_min_size(Pos2::ZERO, Vec2::splat(24.0))")
    add("    }")
    add("")
    add("    /// Every glyph in the three tables, as `(name, glyph)` pairs.")
    add("    fn every() -> Vec<(&'static str, Glyph)> {")
    add("        tables()")
    add("            .iter()")
    add("            .flat_map(|table| table.iter().copied())")
    add("            .collect()")
    add("    }")
    add("")
    add("    /// The stroke widths a glyph draws with, at `size` points square.")
    add("    fn stroke_widths(glyph: Glyph, size: f32) -> Vec<f32> {")
    add("        let rect = Rect::from_min_size(Pos2::ZERO, Vec2::splat(size));")
    add("        parts(glyph, fit(glyph, rect), Color32::WHITE)")
    add("            .into_iter()")
    add("            .filter_map(|shape| match shape {")
    add("                Shape::Path(path) if path.stroke.width > 0.0 => Some(path.stroke.width),")
    add("                _ => None,")
    add("            })")
    add("            .collect()")
    add("    }")
    add("")
    add("    #[test]")
    add("    fn the_whole_reference_is_here() {")
    add("        // The same three counts the iced generator asserts, because both walk")
    add("        // the same directories: a glyph this file quietly skipped would be an")
    add("        // icon the egui window cannot draw, and nothing else would notice.")
    add(f"        assert_eq!(ALL.len(), {numbers['all']});")
    add(f"        assert_eq!(TAG_CATEGORIES.len(), {numbers['categories']});")
    add(f"        assert_eq!(TAG_LOADERS.len(), {numbers['loaders']});")
    add(f"        assert_eq!(every().len(), {numbers['glyphs']}, \"the glyph count moved\");")
    add("        let mut names: Vec<&str> = every().iter().map(|(name, _)| *name).collect();")
    add("        names.sort_unstable();")
    add("        let mut unique = names.clone();")
    add("        unique.dedup();")
    add("        assert_eq!(names, unique, \"two glyphs share a name\");")
    add("        for (_, glyph) in every() {")
    add("            assert!(!glyph.name().is_empty(), \"{glyph:?} is in no table\");")
    add("            assert_eq!(Glyph::by_name(glyph.name()), Some(glyph));")
    add("        }")
    add("    }")
    add("")
    add("    #[test]")
    add("    fn the_enum_matches_the_iced_generator_variant_for_variant() {")
    add("        // The two files are meant to be read side by side, which only works if")
    add("        // the variants are the same names in the same order. The iced file is")
    add("        // read at compile time rather than copied here, so this fails the day")
    add("        // the two generators disagree instead of the day somebody notices.")
    add("        let iced = include_str!(\"../../palantir-desktop/src/icons_gen.rs\");")
    add("        let mut expected: Vec<String> = Vec::new();")
    add("        let mut inside = false;")
    add("        for line in iced.lines() {")
    add("            if line == \"pub enum Glyph {\" {")
    add("                inside = true;")
    add("                continue;")
    add("            }")
    add("            if !inside {")
    add("                continue;")
    add("            }")
    add("            if line == \"}\" {")
    add("                break;")
    add("            }")
    add("            let trimmed = line.trim();")
    add("            if trimmed.starts_with(\"///\") || trimmed.is_empty() {")
    add("                continue;")
    add("            }")
    add("            if let Some(name) = trimmed.strip_suffix(',') {")
    add("                expected.push(name.to_owned());")
    add("            }")
    add("        }")
    add("        let actual: Vec<String> = every()")
    add("            .iter()")
    add("            .map(|(_, glyph)| format!(\"{glyph:?}\"))")
    add("            .collect();")
    add("        assert!(!expected.is_empty(), \"the iced enum was not found\");")
    add("        assert_eq!(actual.len(), expected.len(), \"the variant count moved\");")
    add("        assert_eq!(actual, expected, \"the variant names or their order moved\");")
    add("    }")
    add("")
    add("    #[test]")
    add("    fn every_glyph_produces_at_least_one_shape() {")
    add("        for (name, glyph) in every() {")
    add("            let shapes = parts(glyph, fit(glyph, sample_box()), Color32::WHITE);")
    add("            assert!(!shapes.is_empty(), \"{name} draws no shape\");")
    add("            assert_eq!(")
    add("                shapes.len(),")
    add("                glyph.subpaths().len(),")
    add("                \"{name} lost or gained a shape on the way to the painter\"")
    add("            );")
    add("            for shape in &shapes {")
    add("                match shape {")
    add("                    Shape::Path(path) => {")
    add("                        assert!(path.points.len() >= 2, \"{name} has a degenerate path\");")
    add("                        assert!(")
    add("                            path.stroke.width > 0.0 || path.fill != Color32::TRANSPARENT,")
    add("                            \"{name} has a shape that paints nothing\"")
    add("                        );")
    add("                        for point in &path.points {")
    add("                            assert!(")
    add("                                point.x.is_finite() && point.y.is_finite(),")
    add("                                \"{name} has a point that is not a number\"")
    add("                            );")
    add("                        }")
    add("                    }")
    add("                    other => panic!(\"{name} produced something that is not a path: {other:?}\"),")
    add("                }")
    add("            }")
    add("        }")
    add("    }")
    add("")
    add("    /// The recorded shape count for every glyph, in the order the enum has them.")
    add("    ///")
    add("    /// **This table is a change detector, not a truth.** Nothing in the world")
    add("    /// says `Glyph::Home` has two shapes; what the table says is that it had two")
    add("    /// when this file was generated. A change to the flattening tolerance, to")
    add("    /// the subpath splitting, or to a vendored SVG then fails here, with the")
    add("    /// glyph named, rather than showing up as a picture that is subtly different")
    add("    /// in a window nobody happens to be comparing.")
    add("    ///")
    add("    /// Two glyphs drawing the same number of shapes is normal and is not what")
    add("    /// this asserts -- 441 icons drawn from one vocabulary collide constantly --")
    add("    /// which is why the assertion is the whole table rather than `the counts are")
    add("    /// all different`. The distinct-count check below is only there so that the")
    add("    /// table cannot pass by having collapsed to one value.")
    add("    const PATH_COUNTS: &[(Glyph, u32)] = &[")
    for name, rust, count in shape_counts:
        add(f"        (Glyph::{rust}, {count}),")
    add("    ];")
    add("")
    add("    #[test]")
    add("    fn the_shape_counts_are_the_recorded_ones() {")
    add("        assert_eq!(PATH_COUNTS.len(), every().len(), \"the table is not every glyph\");")
    add("        let mut total = 0u32;")
    add("        for (glyph, recorded) in PATH_COUNTS {")
    add("            let drawn = parts(*glyph, fit(*glyph, sample_box()), Color32::WHITE).len() as u32;")
    add("            assert_eq!(drawn, *recorded, \"{glyph:?} draws {drawn} shapes, not {recorded}\");")
    add("            total += drawn;")
    add("        }")
    add(f"        assert_eq!(total, {total_shapes}, \"the shape total moved\");")
    add("        let distinct: std::collections::BTreeSet<u32> =")
    add("            PATH_COUNTS.iter().map(|(_, count)| *count).collect();")
    add("        assert!(distinct.len() >= 20, \"the counts have collapsed: {distinct:?}\");")
    add("    }")
    add("")
    add("    #[test]")
    add("    fn an_icon_fits_its_box_and_touches_it() {")
    add("        // The SVG default, `xMidYMid meet`, checked over every glyph rather")
    add("        // than asserted for one: the icon is never drawn larger than the box in")
    add("        // either direction, never smaller in both, touches one pair of edges,")
    add("        // and has equal margins on the axes that do not touch.")
    add("        const SLACK: f32 = 1e-4;")
    add("        for (name, glyph) in every() {")
    add("            let (width, height) = glyph.view_box();")
    add("            for size in [16.0, 20.0, 24.0, 48.0] {")
    add("                let placement =")
    add("                    fit(glyph, Rect::from_min_size(Pos2::ZERO, Vec2::splat(size)));")
    add("                let drawn_width = width * placement.scale;")
    add("                let drawn_height = height * placement.scale;")
    add("                assert!(drawn_width <= size + SLACK, \"{name} wide in {size}\");")
    add("                assert!(drawn_height <= size + SLACK, \"{name} tall in {size}\");")
    add("                let touches = (drawn_width - size).abs() < SLACK")
    add("                    || (drawn_height - size).abs() < SLACK;")
    add("                assert!(touches, \"{name} does not reach the edge of {size}\");")
    add("                assert!(")
    add("                    (placement.offset.x * 2.0 - (size - drawn_width)).abs() < SLACK,")
    add("                    \"{name} is not centred horizontally in {size}\"")
    add("                );")
    add("                assert!(")
    add("                    (placement.offset.y * 2.0 - (size - drawn_height)).abs() < SLACK,")
    add("                    \"{name} is not centred vertically in {size}\"")
    add("                );")
    add("                assert!(")
    add("                    placement.offset.x >= -SLACK && placement.offset.y >= -SLACK,")
    add("                    \"{name} is off its box\"")
    add("                );")
    add("            }")
    add("        }")
    add("    }")
    add("")
    add("    #[test]")
    add("    fn a_twenty_four_unit_icon_is_drawn_identity_at_twenty_four_pixels() {")
    add("        // 423 of the 441 declare `viewBox=\"0 0 24 24\"`, and the rail draws")
    add("        // them at 24 px. Scale 1 with no offset is what makes the stroke widths")
    add("        // in the generated tables the reference's own numbers.")
    add("        let identity = fit(Glyph::Home, sample_box());")
    add("        assert_eq!(identity.scale, 1.0);")
    add("        assert_eq!(identity.offset, Vec2::ZERO);")
    add("        let two_thirds = fit(Glyph::Home, Rect::from_min_size(Pos2::ZERO, Vec2::splat(16.0)));")
    add("        assert!((two_thirds.scale - 2.0 / 3.0).abs() < 1e-4);")
    add("        assert_eq!(two_thirds.offset, Vec2::ZERO);")
    add("    }")
    add("")
    add("    #[test]")
    add("    fn a_stroke_scales_with_the_fit_factor() {")
    add("        // The half of the scaling that is easy to lose. epaint strokes the")
    add("        // points it is handed at the width it is handed, so a caller that")
    add("        // scaled only the positions would draw a 16 px icon with a 24 px icon's")
    add("        // pen: every edge a third too heavy. `parts` multiplies the width by")
    add("        // the fit scale, and this is the ratio it has to produce.")
    add("        let at_24 = stroke_widths(Glyph::Home, 24.0);")
    add("        let at_48 = stroke_widths(Glyph::Home, 48.0);")
    add("        let at_20 = stroke_widths(Glyph::Home, 20.0);")
    add("        assert!(!at_24.is_empty(), \"the home icon draws no stroke\");")
    add("        assert_eq!(at_24.len(), at_48.len());")
    add("        assert_eq!(at_24.len(), at_20.len());")
    add("        for index in 0..at_24.len() {")
    add("            let (small, large, middle) = (at_24[index], at_48[index], at_20[index]);")
    add("            assert!(")
    add("                (large - small * 2.0).abs() < 1e-4,")
    add("                \"{small} at 24 px is {large} at 48 px\"")
    add("            );")
    add("            assert!(")
    add("                (middle - small * 20.0 / 24.0).abs() < 1e-4,")
    add("                \"{small} at 24 px is {middle} at 20 px\"")
    add("            );")
    add("        }")
    add("    }")
    add("")
    add("    #[test]")
    add("    fn drawing_into_a_rect_with_no_room_does_not_panic() {")
    add("        // A collapsed panel hands a widget a zero-by-zero rectangle, and the")
    add("        // frame after a resize can hand it an inverted one. Neither is an")
    add("        // error: `fit` answers with the degenerate scale and the shapes land")
    add("        // behind a zero-sized clip. This drives every glyph through both, and")
    add("        // through the cached path as well, so the case is exercised rather")
    add("        // than reasoned about.")
    add("        let empty = Rect::from_min_size(Pos2::new(17.0, 23.0), Vec2::ZERO);")
    add("        let inverted = Rect::from_min_max(Pos2::new(10.0, 10.0), Pos2::new(-10.0, -10.0));")
    add("        let ctx = egui::Context::default();")
    add("        let _ = ctx.run_ui(egui::RawInput::default(), |ui| {")
    add("            for (_, glyph) in every() {")
    add("                draw(ui.painter(), glyph, empty, Color32::WHITE);")
    add("                let _ = cached(ui.painter(), glyph, empty, Color32::WHITE);")
    add("                draw(ui.painter(), glyph, inverted, Color32::WHITE);")
    add("            }")
    add("        });")
    add("    }")
    add("")
    add("    #[test]")
    add("    fn a_cached_glyph_is_invalidated_by_glyph_size_position_and_ink() {")
    add("        // The cache is only correct if a change to any of the four clears it,")
    add("        // and none of the four can be observed through a painter in a unit")
    add("        // test. This is the gate for the cache: an icon that kept the rest")
    add("        // ink through a hover, or a row's old size, would be a stale picture")
    add("        // nobody would think to look for.")
    add("        let home = fit(Glyph::Home, sample_box());")
    add("        let base = cache_key(Glyph::Home, home, Color32::WHITE);")
    add("        assert_eq!(base, cache_key(Glyph::Home, home, Color32::WHITE), \"same inputs\");")
    add("        let compass = fit(Glyph::Compass, sample_box());")
    add("        assert_ne!(base, cache_key(Glyph::Compass, compass, Color32::WHITE), \"a glyph\");")
    add("        let bigger = fit(Glyph::Home, Rect::from_min_size(Pos2::ZERO, Vec2::splat(48.0)));")
    add("        assert_ne!(base, cache_key(Glyph::Home, bigger, Color32::WHITE), \"a size\");")
    add("        let moved =")
    add("            fit(Glyph::Home, Rect::from_min_size(Pos2::new(1.0, 0.0), Vec2::splat(24.0)));")
    add("        assert_ne!(base, cache_key(Glyph::Home, moved, Color32::WHITE), \"a position\");")
    add("        assert_ne!(")
    add("            base,")
    add("            cache_key(Glyph::Home, home, Color32::from_rgb(255, 0, 0)),")
    add("            \"an ink\"")
    add("        );")
    add("    }")
    add("")
    add("    #[test]")
    add("    fn the_fills_epaint_can_draw_are_closed_and_have_no_stroke() {")
    add("        // `PathShape::convex_polygon` is what a filled subpath becomes: closed,")
    add("        // filled, and with the empty stroke so epaint does not also outline it.")
    add("        // A fill that arrived open, or with a stroke, would be the wrong")
    add("        // primitive rather than the wrong geometry, which is the kind of thing")
    add("        // that renders and looks nearly right.")
    add("        let mut filled = 0;")
    add("        for (name, glyph) in every() {")
    add("            for subpath in glyph.subpaths() {")
    add("                if !subpath.filled {")
    add("                    continue;")
    add("                }")
    add("                filled += 1;")
    add("                assert!(subpath.closed, \"{name} has an open fill\");")
    add("                assert!(subpath.points.len() >= 3, \"{name} has a fill with no area\");")
    add("            }")
    add("        }")
    add(f"        assert_eq!(filled, {sum(1 for _, _, _, _, shapes in geometry for _, _, element in shapes if element.filled)}, \"the filled shape count moved\");")
    add("    }")
    add("")
    add("    #[test]")
    add("    fn the_stroke_widths_that_are_not_the_sets_own_two_are_the_ones_it_names() {")
    add("        // Nearly the whole set strokes at 2; a handful of glyphs declare their")
    add("        // own because their art is deliberately heavier or finer, and `loader`")
    add("        // reaches 2 through its own transform's scale factor. The exceptions")
    add("        // are written down so a sixth one appearing upstream fails here.")
    add("        let mut widths: Vec<String> = every()")
    add("            .into_iter()")
    add("            .flat_map(|(name, glyph)| {")
    add("                glyph")
    add("                    .subpaths()")
    add("                    .iter()")
    add("                    .map(move |subpath| format!(\"{name}:{}\", subpath.stroke_width))")
    add("            })")
    add("            .filter(|entry| {")
    add("                let width = entry.rsplit(':').next().and_then(|text| text.parse::<f32>().ok());")
    add("                match width {")
    add("                    Some(value) => (value - 2.0).abs() > 0.01,")
    add("                    None => true,")
    add("                }")
    add("            })")
    add("            .collect();")
    add("        widths.sort_unstable();")
    add("        widths.dedup();")
    add(f"        let expected: Vec<&str> = vec![{', '.join(chr(34) + w + chr(34) for w in width_exceptions)}];")
    add("        assert_eq!(widths, expected, \"the set of glyphs that do not stroke at width 2 moved\");")
    add("        assert!(!widths.is_empty(), \"the exceptions list is empty, so this proves nothing\");")
    add("    }")
    add("")
    add("    #[test]")
    add("    fn the_inks_that_are_not_inherited_are_the_ones_it_names() {")
    add("        // `currentColor` is nearly every shape in the set; a couple are")
    add("        // painted a theme token or a literal instead, and those are the ones a")
    add("        // reader would not expect to take the colour of the button they sit in.")
    add("        let mut fixed: Vec<&str> = every()")
    add("            .into_iter()")
    add("            .filter(|(_, glyph)| {")
    add("                glyph")
    add("                    .subpaths()")
    add("                    .iter()")
    add("                    .any(|subpath| matches!(subpath.ink, Ink::Fixed(..)))")
    add("            })")
    add("            .map(|(name, _)| name)")
    add("            .collect();")
    add("        fixed.sort_unstable();")
    add(f"        let expected: Vec<&str> = vec![{', '.join(chr(34) + n + chr(34) for n in fixed_colours)}];")
    add("        assert_eq!(fixed, expected, \"the glyphs with a literal ink moved\");")
    add("        let mut tokens: Vec<String> = every()")
    add("            .into_iter()")
    add("            .flat_map(|(name, glyph)| {")
    add("                glyph")
    add("                    .subpaths()")
    add("                    .iter()")
    add("                    .filter_map(move |subpath| match subpath.ink {")
    add("                        Ink::Token(token) => Some(format!(\"{name}:{token}\")),")
    add("                        _ => None,")
    add("                    })")
    add("            })")
    add("            .collect();")
    add("        tokens.sort_unstable();")
    add("        tokens.dedup();")
    add(f"        let expected: Vec<&str> = vec![{', '.join(chr(34) + t + chr(34) for t in tokens)}];")
    add("        assert_eq!(tokens, expected, \"the tokens the set asks for moved\");")
    add("    }")
    add("}")
    add("")

    text = "\n".join(lines)

    report = [
        f"glyphs       {len(icons)}",
        "  " + "  ".join(
            f"{source['table']} {len(sets[source['table']])}" for source in gen_icons.SOURCES
        ),
        f"shapes       {total_shapes}",
        f"points       {total_points}",
        f"elements     {numbers['elements']} in total, {numbers['no_paint']} paint nothing",
        f"largest      {largest[0]} ({largest[2]} shapes)",
        f"filled       {numbers['filled_elements']} elements over {numbers['filled_glyphs']} glyphs",
        f"even-odd     {len(even_odds)}: {', '.join(even_odds) if even_odds else '-'}",
        "unexpressible "
        + (f"{len(not_expressible)}: "
           + ", ".join(f"{name}#{index} ({reason})" for name, index, reason, _ in not_expressible)
           if not_expressible else "-"),
        f"no paint     {len(no_paint)}: {', '.join(sorted(no_paint)) if no_paint else '-'}",
        f"refused      {len(refused)}: {', '.join(name for name, _ in sorted(refused)) or '-'}",
        f"off-width    {len(width_exceptions)}: {', '.join(width_exceptions) if width_exceptions else '-'}",
        f"tokens       {', '.join(tokens) if tokens else '-'}",
        f"fixed ink    {', '.join(fixed_colours) if fixed_colours else '-'}",
        f"fill glyphs  {', '.join(filled_glyphs) if filled_glyphs else '-'}",
    ]

    if args.check:
        current = OUT.read_text(encoding="utf-8") if OUT.is_file() else ""
        if current != text:
            print(f"{OUT.relative_to(ROOT)} is not what this tool emits.", file=sys.stderr)
            print("Run `python tools/gen_icons_egui.py` and commit the result.", file=sys.stderr)
            return 1
        print("egui icon generation is byte-identical")
        return 0

    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text(text, encoding="utf-8", newline="\n")
    if args.report:
        for line in report:
            print(line)
    print(f"wrote {OUT.relative_to(ROOT)} "
          f"({text.count(chr(10)) + 1} lines, {len(text.encode('utf-8'))} bytes)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
