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
  a subpath is independent of its siblings -- and it is why each stroked subpath
  can be given its own round joins without disturbing the pen of its siblings.
* **Triangulate the fills.** `PathShape` has no fill rule and epaint fills it with
  a triangle fan from the first point, so a concave outline is painted outside
  itself and a hole is painted solid. Filled elements are triangulated here --
  ear clipping with hole bridging, honouring the element's own `fill-rule` -- and
  emitted as `egui::Shape::Mesh`. See "Triangulating a filled element" below and
  `docs/PARITY-LOG.md`, "Two fidelity gaps in the icon set".
* **Round the joins and caps.** epaint miters every corner and butts every line
  end. A stroke turns by more than `JOIN_DEGREES` at a corner, so a disc of the
  stroke's width is emitted at that vertex, and a half-disc at each open end;
  together they are one small mesh drawn over the path. Why a disc rather than a
  miter is the whole point: the reference declares round joins and caps, and
  epaint has no attribute that would ask for them. Same parity-log entry.

    python tools/gen_icons_egui.py            # write the egui crate's src/icons.rs
    python tools/gen_icons_egui.py --check    # fail if the file on disk is not this
    python tools/gen_icons_egui.py --report   # glyph/shape/triangle counts and the area check

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
import os
import sys
from dataclasses import dataclass, field
from pathlib import Path

# The parser lives in the sibling tool; put its directory on the path rather than
# copying a function out of it. `gen_icons` has no import-time side effects (its
# `main()` is behind `if __name__`), so importing it only defines things.
TOOLS = Path(__file__).resolve().parent
if str(TOOLS) not in sys.path:
    sys.path.insert(0, str(TOOLS))

import gen_icons  # noqa: E402  -- after the path fix-up above, and it is the point

ROOT = gen_icons.ROOT

# The crate this writes into, by the names it has had. The egui crate was renamed
# while this tool was being written (`palantir-egui` -> `sift-ui`), and a hard-coded
# destination is a megabyte of generated file in a directory nobody reads the day it
# is renamed again. The first name that exists wins; there is only ever one.
EGUI_CRATE_NAMES = ("sift-ui", "palantir-egui")

# Where the iced file this one mirrors lives, asked of the generator that writes it
# rather than written out here. The tree has been renamed once already and a second
# literal path is a second thing to forget; `gen_icons.OUT` is the answer the iced
# generator itself uses.
ICED = gen_icons.OUT
ICED_FROM_ROOT = ICED.relative_to(ROOT).as_posix()


def output() -> Path:
    """The file to write: the egui crate's `src/icons.rs`.

    `include_str!` in the generated test is resolved from the directory of the file
    containing it, so the path to the iced file is computed against whichever
    directory this lands in. The path is returned from here rather than
    `ICED_INCLUDE` being a module constant for the same reason `OUT` is not one:
    importing this module must not decide where the tree is.
    """
    for name in EGUI_CRATE_NAMES:
        crate = ROOT / "crates" / name
        if crate.is_dir():
            return crate / "src" / "icons.rs"
    raise SystemExit(
        "no egui crate under crates/ (looked for " + " and ".join(EGUI_CRATE_NAMES) + ")"
    )

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

# How short a stroked subpath has to be, as a fraction of the view box's long side,
# before it is the reference's *dot* rather than a line.
#
# The set draws a dot as a zero-length line with a round cap -- `M12 12h.01` -- and
# the cap is the whole of what you see. A thousandth of the view box is 0.024 units
# on a 24-unit icon: a fiftieth of a pixel at 24 px. The shortest subpath in the set
# that is *not* that idiom is 0.41 units, seventeen times this limit, so the two are
# not close enough to confuse. See `disc`.
DOT_FRACTION = 1e-3

# How sharply a stroked subpath has to turn before the corner is rounded, in
# degrees, and how many sides the disc that rounds it has.
#
# `stroke-linejoin="round"` replaces a corner with an arc of the stroke's radius,
# so the disc is only needed where the corner is visible at icon size: a 20-degree
# turn moves the outer corner by a hundredth of the stroke's width, well under
# epaint's half-pixel feathering. Flattened curves are the reason for a threshold
# rather than "every vertex": a tight arc is many small turns and rounding each of
# them would add a mesh per flattened point. Eight sides put every disc vertex
# within 7.6% of the radius of the circle it stands in for -- 0.08 px for a 2-unit
# pen at 24 px.
JOIN_DEGREES = 20.0
JOIN_SIDES = 8

# Segments in the half-disc that stands in for a round line cap. Half of a
# twelve-sided disc: the cap is a 180-degree arc plus its diameter, and the
# diameter is the stroke's own butt end, so only the arc has to be approximated.
CAP_SIDES = 6

# The absolute tolerance for "is this triangle degenerate", in view-box units
# squared. Coordinates are emitted at a thousandth of a unit, so a triangle with
# less than this area is round-off, not geometry.
AREA_EPSILON = 1e-12

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


def disc(center: tuple[float, float], radius: float, tolerance: float) -> list[tuple[float, float]]:
    """A circle as the regular polygon whose sagitta is within `tolerance`.

    The reference draws a dot as a *zero-length line with a round cap*, which is a
    disc of the stroke's width -- `M12 12h.01` is the idiom, and the cap is the dot.
    epaint has no round caps, so the only way to draw what the reference draws is to
    fill the disc, and a disc is not a `PathShape`: the polygon that stands in for it
    is. Its vertex count is solved from the sagitta `r (1 - cos(pi / n))` rather than
    fixed, so a 3-unit dot is as round as a 1-unit one.
    """
    if radius <= 0.0:
        return []
    ratio = 1.0 - min(tolerance / radius, 1.0)
    steps = max(12, math.ceil(math.pi / math.acos(ratio)))
    steps = min(steps, 64)
    return [
        (center[0] + radius * math.cos(math.tau * index / steps),
         center[1] + radius * math.sin(math.tau * index / steps))
        for index in range(steps)
    ]


def bounding_extent(points: list[tuple[float, float]]) -> float:
    """The diagonal of the points' bounding box, which is 0 for a single point."""
    if not points:
        return 0.0
    xs = [point[0] for point in points]
    ys = [point[1] for point in points]
    return math.hypot(max(xs) - min(xs), max(ys) - min(ys))


@dataclass
class Piece:
    """One drawable piece of an icon, and how to paint it.

    A `Piece` is one `Shape` in the generated file: a stroke polyline, a filled
    mesh, a dot, or the small mesh of round joins and caps that belongs to the
    stroke just before it. `round` is how many discs that last kind holds and
    `caps` how many of them are half-discs; both are zero for everything else.
    """

    points: list[tuple[float, float]]
    closed: bool
    dot: bool = False
    triangles: list[int] = field(default_factory=list)
    round: int = 0
    caps: int = 0
    holes: int = 0
    concave: bool = False
    outer_area: float = 0.0
    hole_area: float = 0.0
    mesh_area: float = 0.0
    outer_triangles: int = 0


def element_contours(element, tolerance: float) -> list[tuple[list[tuple[float, float]], bool]]:
    """One element's polylines and whether each closes, in document order.

    An element the reference paints nothing for (`Cmd::NoPaint`, the clamp path an
    exported icon carries) has no geometry and comes back empty: SVG draws nothing
    for it, and so does this. `closed` is true for a subpath that ends in `Close`
    *and* for every filled one, because a fill closes its contour whether the SVG
    says so or not.
    """
    out: list[tuple[list[tuple[float, float]], bool]] = []
    points: list[tuple[float, float]] = []
    current = (0.0, 0.0)
    closed = False

    def flush() -> None:
        nonlocal points, closed
        cleaned = tidy(points, closed or element.filled)
        if cleaned:
            out.append((cleaned, closed))
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


@dataclass
class RoundTally:
    """The joins and caps the strokes asked for, against the ones that were built.

    This is the generator's refusal to emit a silently unrounded set: every
    stroked subpath reports its own expected count to `round_pieces`, and `gate`
    checks the whole run after the last icon. A code path that stops inserting
    discs -- a changed turn test, a tolerance that swallowed a corner -- fails the
    run with the total, rather than producing a file that is subtly more mitered
    than the one before it.
    """

    expected: int = 0
    produced: int = 0
    strokes: int = 0

    def gate(self) -> None:
        if self.strokes and not self.produced:
            raise SystemExit(
                f"{self.strokes} stroked subpaths needed {self.expected} round joins/caps, "
                "but none were produced; refusing to emit a mitered set"
            )
        if self.produced < self.expected:
            raise SystemExit(
                f"{self.expected} round joins/caps were expected but only {self.produced} "
                "were produced; refusing to emit an under-rounded set"
            )


def turn_degrees(previous, current, following) -> float:
    """How sharply a polyline turns at `current`, in degrees."""
    incoming = (current[0] - previous[0], current[1] - previous[1])
    outgoing = (following[0] - current[0], following[1] - current[1])
    first = math.hypot(*incoming)
    second = math.hypot(*outgoing)
    if first == 0.0 or second == 0.0:
        return 0.0
    cosine = (incoming[0] * outgoing[0] + incoming[1] * outgoing[1]) / (first * second)
    return math.degrees(math.acos(max(-1.0, min(1.0, cosine))))


def join_disc(center, radius: float) -> list[tuple[float, float]]:
    """The polygon that stands in for a round join at `center`.

    Why a disc rather than a miter: the reference declares
    `stroke-linejoin="round"` on 413 of the 441 glyphs, and epaint has no cap or
    join attribute at all -- its stroker miters every corner. A disc of the
    stroke's radius, centred on the vertex, is the arc SVG would have drawn there;
    it is painted over the miter, and the miter's tip outside the disc is the one
    thing this cannot remove. Eight sides is `JOIN_SIDES`; a regular polygon is
    convex, so its fan is exact.
    """
    return [
        (center[0] + radius * math.cos(math.tau * side / JOIN_SIDES),
         center[1] + radius * math.sin(math.tau * side / JOIN_SIDES))
        for side in range(JOIN_SIDES)
    ]


def round_cap(center, radius: float, outward) -> list[tuple[float, float]]:
    """The half-disc that stands in for a round cap at an open end.

    Half, not whole: the inner half of a disc at the endpoint is already under the
    stroke's own butt end, and a full disc there would double-blend when the
    element carries an opacity below one. The diameter is the stroke's end, so only
    the 180-degree arc is approximated (`CAP_SIDES` segments).
    """
    base = math.atan2(outward[1], outward[0])
    return [
        (center[0] + radius * math.cos(base + math.pi * 0.5 - math.pi * step / CAP_SIDES),
         center[1] + radius * math.sin(base + math.pi * 0.5 - math.pi * step / CAP_SIDES))
        for step in range(CAP_SIDES + 1)
    ]


def round_pieces(points: list[tuple[float, float]], closed: bool, width: float,
                 tally: RoundTally, label: str
                 ) -> tuple[list[list[tuple[float, float]]], int, int]:
    """The discs and half-discs that round one stroked subpath's corners and ends.

    Every interior vertex -- every vertex, for a closed subpath, including the one
    where it closes -- whose turn is sharper than `JOIN_DEGREES` gets a disc. Every
    *open* end gets a half-disc; a closed subpath has no ends, so it gets none.
    The returned list is what the caller turns into one small mesh; the two
    integers are joins and caps, so the report can tell them apart.
    """
    radius = width * 0.5
    polygons: list[list[tuple[float, float]]] = []
    joins = 0
    caps = 0
    if radius <= 0.0:
        return polygons, joins, caps
    count = len(points)
    if closed:
        corners = range(count)
    else:
        corners = range(1, count - 1)
        if count >= 2:
            for center, inward in ((points[0], points[1]),
                                   (points[-1], points[-2])):
                outward = (center[0] - inward[0], center[1] - inward[1])
                length = math.hypot(*outward)
                if length == 0.0:
                    continue
                polygons.append(round_cap(center, radius,
                                          (outward[0] / length, outward[1] / length)))
                caps += 1
    for index in corners:
        previous = points[index - 1]
        current = points[index]
        following = points[(index + 1) % count]
        if turn_degrees(previous, current, following) > JOIN_DEGREES:
            polygons.append(join_disc(current, radius))
            joins += 1
    expected = joins + caps
    tally.expected += expected
    tally.produced += len(polygons)
    if expected:
        tally.strokes += 1
    if expected and not polygons:
        raise SystemExit(
            f"{label}: a stroked subpath turned at {joins} corner(s) and has {caps} "
            "open end(s), but no round pieces were produced"
        )
    if len(polygons) != expected:
        raise SystemExit(
            f"{label}: expected {expected} round pieces for one stroked subpath, "
            f"built {len(polygons)}"
        )
    return polygons, joins, caps


def merge_convex(polygons: list[list[tuple[float, float]]]
                 ) -> tuple[list[tuple[float, float]], list[int]]:
    """Concatenate convex polygons into one mesh.

    One mesh per stroked subpath rather than one shape per disc: the discs share an
    ink and an order, both are already over the path, and a glyph with twenty
    corners would otherwise cost twenty paint-list entries to say what one
    triangle list says. The vertices are not welded -- two discs that touch keep
    their own points -- because welding would need a tolerance and buys nothing.
    """
    points: list[tuple[float, float]] = []
    triangles: list[int] = []
    for polygon in polygons:
        start = len(points)
        points.extend(polygon)
        triangles.extend(start + index for index in fan_triangles(len(polygon)))
    return points, triangles


def subpaths(element, tolerance: float, dot_limit: float,
             tally: RoundTally, label: str) -> list[Piece]:
    """One element's geometry as the pieces egui can express.

    Filled elements become triangulated meshes -- see `triangulate_fill` -- because
    a `PathShape` fill cannot leave a hole. Stroked elements become one polyline
    each, plus, where the line turns or ends, one mesh of discs and half-discs that
    supplies the round joins and caps the reference declares and epaint lacks. A
    stroked subpath shorter than `dot_limit` is the reference's dot idiom (`M12
    12h.01`, a zero-length line whose round cap is the whole picture) and becomes
    the disc that cap painted; that conversion predates the round-cap work and is
    left exactly as it was.
    """
    if element.commands and element.commands[0][0] == "NoPaint":
        return []
    contours = element_contours(element, tolerance)
    if element.filled:
        filled = [points for points, _closed in contours if len(points) >= 3]
        return [
            Piece(points=mesh.points, closed=True, triangles=mesh.triangles,
                  holes=mesh.holes, concave=mesh.concave,
                  outer_area=mesh.outer_area, hole_area=mesh.hole_area,
                  mesh_area=mesh.mesh_area, outer_triangles=mesh.outer_triangles)
            for mesh in triangulate_fill(filled, element.even_odd, label)
        ]

    pieces: list[Piece] = []
    for points, closed in contours:
        if bounding_extent(points) <= dot_limit:
            middle = (
                sum(point[0] for point in points) / len(points),
                sum(point[1] for point in points) / len(points),
            )
            blob = disc(middle, element.stroke_width * 0.5, tolerance)
            if blob:
                pieces.append(Piece(points=blob, closed=True, dot=True,
                                    triangles=fan_triangles(len(blob))))
            continue
        if len(points) < 2:
            continue
        pieces.append(Piece(points=points, closed=closed))
        round_polygons, joins, caps = round_pieces(points, closed, element.stroke_width,
                                                   tally, label)
        if round_polygons:
            rounded_points, triangles = merge_convex(round_polygons)
            pieces.append(Piece(points=rounded_points, closed=True, triangles=triangles,
                                round=joins + caps, caps=caps))
    return pieces


# --------------------------------------------------------------------------
# Contour predicates
# --------------------------------------------------------------------------


def signed_area(points: list[tuple[float, float]]) -> float:
    """The shoelace area. Its sign is the contour's winding direction."""
    total = 0.0
    count = len(points)
    for index in range(count):
        x1, y1 = points[index]
        x2, y2 = points[(index + 1) % count]
        total += x1 * y2 - x2 * y1
    return 0.5 * total


def fan_covers(points: list[tuple[float, float]]) -> bool:
    """Whether epaint's triangle fan covers exactly this contour.

    epaint fills a path with the triangles `(0, i-1, i)` taken from the first point
    and documents the result as convex-only. Convexity is sufficient and not
    necessary: the fan is exact for any contour that is star-shaped as seen from
    its first point, and a fan triangle leaves the contour only where an edge turns
    away from that point. That weaker test is the one used here, because it is what
    the tessellator actually draws.

    Kept after the move to meshes because it is the measurement of which elements
    the old `PathShape` fill could not draw; `--report` still names them. The
    tolerance is a perpendicular distance, so it does not move with the icon's
    units, and it is far below the emitted coordinates' precision.
    """
    area = signed_area(points)
    if area == 0.0:
        return False
    sign = 1.0 if area > 0 else -1.0
    origin = points[0]
    count = len(points)
    for index in range(count):
        a, b = points[index], points[(index + 1) % count]
        edge_x, edge_y = b[0] - a[0], b[1] - a[1]
        length = math.hypot(edge_x, edge_y)
        if length == 0.0:
            continue
        apart = edge_x * (origin[1] - a[1]) - edge_y * (origin[0] - a[0])
        if sign * apart / length < -1e-3:
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


def orientation(a, b, c) -> int:
    value = (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
    if abs(value) < 1e-9:
        return 0
    return 1 if value > 0 else -1


def on_segment(a, b, c) -> bool:
    return (min(a[0], b[0]) - 1e-9 <= c[0] <= max(a[0], b[0]) + 1e-9
            and min(a[1], b[1]) - 1e-9 <= c[1] <= max(a[1], b[1]) + 1e-9)


# --------------------------------------------------------------------------
# Filled elements: why epaint cannot be handed one, and what is done instead
# --------------------------------------------------------------------------
#
# `PathShape` carries a fill colour and no fill rule. epaint tessellates it with
# the triangle fan `(0, i-1, i)` around its first point, which is exact only while
# the contour is star-shaped as seen from that point, and a second, nested contour
# is filled solid instead of cutting a hole. That is measured and recorded in
# `docs/PARITY-LOG.md` ("Two fidelity gaps in the icon set"): 16 glyphs came out as
# blobs. The answer here is to do the tessellation at generation time, where the
# SVG's own `fill-rule` is available, and to emit a `Shape::Mesh` whose triangles
# are already decided. `PathShape` stays for strokes.
#
# THE ALGORITHM is ear clipping with hole bridging -- the standard approach the SVG
# and CSS fill rules are written against, and the shape of the one mapbox's
# `earcut` implements:
#
#   * every contour is wound counter-clockwise, its nesting depth is found by
#     point-in-polygon, and the fill rule decides which contours bound filled
#     ground and which are holes (`evenodd`: even depth is filled; `nonzero`: a
#     non-zero winding number is filled);
#   * each hole is cut into its filled parent by a bridge to the nearest *visible*
#     vertex, turning the polygon into a weakly-simple "keyhole" polygon. The
#     corridor is traversed twice, so it has zero width and contributes no area;
#   * the resulting ring is clipped ear by ear: a convex corner whose triangle
#     contains no other vertex is a triangle, and removing it leaves a smaller
#     simple polygon. Clipping stops at the last three vertices.
#
# The area of the resulting triangles is checked against the outer contour minus
# its holes for every filled element, which is what catches a bad bridge instead of
# letting it draw a wedge of ground the reference does not paint. `--report` prints
# it for each glyph that needed the treatment.


def cross3(origin, first, second) -> float:
    """Twice the signed area of `origin, first, second`; its sign is the turn."""
    return ((first[0] - origin[0]) * (second[1] - origin[1])
            - (first[1] - origin[1]) * (second[0] - origin[0]))


def triangle_area(a, b, c) -> float:
    return 0.5 * abs(cross3(a, b, c))


def counter_clockwise(points: list[tuple[float, float]]) -> list[tuple[float, float]]:
    """The contour wound the way ear clipping wants it, reversing if needed.

    Winding is a property of the reference's path, not of the picture -- SVG's
    fill rules read it -- so it is measured before it is normalised and only the
    normalised copy is clipped.
    """
    return list(points) if signed_area(points) >= 0.0 else list(reversed(points))


def interior_point(points: list[tuple[float, float]]) -> tuple[float, float]:
    """A point just inside a simple contour, whatever way it is wound.

    The midpoint of the longest edge nudged along its inward normal -- the left
    normal for a counter-clockwise contour, the right one for a clockwise one. A
    vertex would sit on the boundary of a sibling contour that touches it, where a
    ray cast is a coin toss; the midpoint of the longest edge is as far from the
    rounding of a short edge as the contour offers.
    """
    count = len(points)
    longest = max(
        range(count),
        key=lambda index: math.dist(points[index], points[(index + 1) % count]),
    )
    first, second = points[longest], points[(longest + 1) % count]
    dx, dy = second[0] - first[0], second[1] - first[1]
    length = math.hypot(dx, dy)
    if length == 0.0:
        return first
    # The left normal of each edge points into a counter-clockwise polygon and out
    # of a clockwise one, so the sign of the contour's area decides which way is in.
    sign = 1.0 if signed_area(points) >= 0.0 else -1.0
    inward = (-dy / length * sign, dx / length * sign)
    step = 1e-4 * max(bounding_extent(points), 1.0)
    middle = ((first[0] + second[0]) * 0.5, (first[1] + second[1]) * 0.5)
    return (middle[0] + inward[0] * step, middle[1] + inward[1] * step)


def nesting(contours: list[list[tuple[float, float]]]) -> tuple[list[int], list[int]]:
    """Each contour's depth, and the index of its immediate container (-1 for none).

    Depth is the number of contours that contain a point just inside this one.
    The icons' filled elements are a handful of contours each, so the quadratic
    probe is cheaper than a sweep line and far easier to be sure of.
    """
    probes = [interior_point(contour) for contour in contours]
    contains = [
        [other_index != index and point_inside(probe, contours[other_index])
         for other_index in range(len(contours))]
        for index, probe in enumerate(probes)
    ]
    depth = [sum(row) for row in contains]
    parent = []
    for index in range(len(contours)):
        containers = [other for other in range(len(contours)) if contains[index][other]]
        parent.append(max(containers, key=lambda other: depth[other]) if containers else -1)
    return depth, parent


def filled_contours(contours: list[list[tuple[float, float]]], even_odd: bool
                    ) -> list[bool]:
    """Which contours bound filled ground, under the element's own `fill-rule`.

    `evenodd` fills where an odd number of contours enclose the point, so the
    nesting parity is the answer. `nonzero` fills where the winding number is not
    zero, so a contour nested the same way as its parent is redundant (still
    filled) and one wound the other way is a hole.
    """
    _, parent = nesting(contours)
    windings = [1 if signed_area(contour) > 0.0 else -1 for contour in contours]
    filled: list[bool] = []
    for index in range(len(contours)):
        if even_odd:
            # `nesting` is not returned by this call; depth is the chain length.
            depth = 0
            ancestor = parent[index]
            while ancestor >= 0:
                depth += 1
                ancestor = parent[ancestor]
            filled.append(depth % 2 == 0)
            continue
        winding = windings[index]
        ancestor = parent[index]
        while ancestor >= 0:
            winding += windings[ancestor]
            ancestor = parent[ancestor]
        filled.append(winding != 0)
    return filled


@dataclass
class Group:
    """One filled region: an outer contour and the holes cut out of it."""

    outer: int
    holes: list[int] = field(default_factory=list)


def fill_groups(contours: list[list[tuple[float, float]]], even_odd: bool,
                label: str) -> list[Group]:
    """The region each filled contour belongs to, holes attached to their parent.

    A contour that is filled but sits inside another filled contour adds no ground
    of its own; its holes still cut the outer region, so they are promoted to the
    nearest filled ancestor. That is what makes an island inside a hole work (a
    `nonzero` path can alternate more than once) and it leaves the `evenodd`
    case -- strictly alternating -- as the simple special case.
    """
    depth, parent = nesting(contours)
    filled = filled_contours(contours, even_odd)
    groups: list[Group] = []
    group_of: list[int] = [-1] * len(contours)
    for index in sorted(range(len(contours)), key=lambda candidate: depth[candidate]):
        if filled[index]:
            # A filled contour inside another filled one adds no ground of its
            # own, so it joins that region and its holes are promoted to it. A
            # filled contour whose parent is a *hole* is an island: the hole
            # separates it from the outer region, so it starts a region of its
            # own. (Nested windings can alternate more than once; `evenodd` can
            # only alternate, which is the simple case.)
            if parent[index] >= 0 and filled[parent[index]]:
                group_of[index] = group_of[parent[index]]
            else:
                group_of[index] = len(groups)
                groups.append(Group(outer=index))
            continue
        ancestor = parent[index]
        while ancestor >= 0 and not filled[ancestor]:
            ancestor = parent[ancestor]
        if ancestor < 0:
            raise SystemExit(f"{label}: a hole outside every filled contour")
        group_of[index] = group_of[ancestor]
        groups[group_of[index]].holes.append(index)
    if not groups:
        raise SystemExit(f"{label}: filled element with no filled contour")
    return groups


def segments_cross(a, b, c, d) -> bool:
    """Whether two segments cross *through* each other, ignoring shared ends.

    A bridge is allowed to end on a vertex -- that is the point of a bridge -- so
    touching must not count as crossing. Only a proper interior intersection does.
    """
    first, second = orientation(a, b, c), orientation(a, b, d)
    third, fourth = orientation(c, d, a), orientation(c, d, b)
    return (first != 0 and second != 0 and third != 0 and fourth != 0
            and first != second and third != fourth)


def segment_is_visible(first, second, outer, holes) -> bool:
    """Can `first`--`second` be a bridge without leaving the filled ground?

    A bridge may land on a vertex (that is the point) but must not cross an edge or
    pass through a vertex of any contour, and its middle must lie in the filled
    region: inside the outer contour and outside every hole.
    """
    if first == second:
        return False
    for contour in [outer, *holes]:
        count = len(contour)
        for index in range(count):
            a, b = contour[index], contour[(index + 1) % count]
            if a in (first, second) or b in (first, second):
                continue
            if segments_cross(first, second, a, b):
                return False
            if orientation(first, second, a) == 0 and on_segment(first, second, a):
                return False
    middle = ((first[0] + second[0]) * 0.5, (first[1] + second[1]) * 0.5)
    if not point_inside(middle, outer):
        return False
    return not any(point_inside(middle, hole) for hole in holes)


def bridge(ring: list[int], hole: list[int], points: list[tuple[float, float]],
           outer: list[tuple[float, float]],
           holes: list[list[tuple[float, float]]]) -> tuple[int, int] | None:
    """A ring position and hole position that can be joined by a visible bridge.

    The hole's rightmost vertex is the usual choice (it is the one whose ray to
    the right leaves the polygon through the fewest edges), and from it the
    nearest visible vertex of the current ring is used -- "bridge to the nearest
    visible vertex". Ties are broken by the sort order, which is stable.
    """
    hole_position = max(range(len(hole)), key=lambda index: (points[hole[index]][0],
                                                             points[hole[index]][1]))
    hole_point = points[hole[hole_position]]
    candidates = sorted(range(len(ring)), key=lambda index: math.dist(points[ring[index]], hole_point))
    for ring_position in candidates:
        if segment_is_visible(hole_point, points[ring[ring_position]], outer, holes):
            return ring_position, hole_position
    return None


def ear_clip(ring: list[int], points: list[tuple[float, float]], label: str
             ) -> list[tuple[int, int, int]]:
    """Triangulate a weakly-simple counter-clockwise ring by clipping ears.

    A vertex is an ear when the triangle of its neighbours turns the same way as
    the ring and no other vertex lies strictly inside it. Removing ears until
    three vertices remain is the classic O(n^2) ear clip; n here is at most a few
    hundred, once, at generation time.

    Bridge corridors make the ring weakly simple: the two bridge vertices appear
    twice. Their ears are zero-area spurs, so the fallback pass removes a
    collinear or repeated vertex instead of failing -- that is what lets a
    keyhole polygon be clipped at all.
    """
    ring = list(ring)
    triangles: list[tuple[int, int, int]] = []
    while len(ring) > 3:
        count = len(ring)
        clipped = False
        for position in range(count):
            previous, current, following = (ring[position - 1], ring[position],
                                            ring[(position + 1) % count])
            if len({previous, current, following}) < 3:
                continue
            if cross3(points[previous], points[current], points[following]) <= AREA_EPSILON:
                continue
            blocked = False
            for other in range(count):
                if other in (position - 1, position, (position + 1) % count):
                    continue
                # Only a reflex (or flat) vertex can invalidate an ear: a convex
                # one inside the triangle is a vertex of the outline the diagonal
                # can safely leave on the other side. This is what lets a
                # keyhole polygon be clipped at all -- the bridge corridor leaves
                # pairs of vertices on every candidate diagonal.
                if cross3(points[ring[other - 1]], points[ring[other]],
                          points[ring[(other + 1) % count]]) > AREA_EPSILON:
                    continue
                if inside_triangle(points[ring[other]], points[previous],
                                   points[current], points[following]):
                    blocked = True
                    break
            if blocked:
                continue
            triangles.append((previous, current, following))
            del ring[position]
            clipped = True
            break
        if clipped:
            continue
        for position in range(len(ring)):
            previous, current, following = (ring[position - 1], ring[position],
                                            ring[(position + 1) % len(ring)])
            if (len({previous, current, following}) < 3
                    or abs(cross3(points[previous], points[current], points[following]))
                    <= AREA_EPSILON):
                del ring[position]
                clipped = True
                break
        if not clipped:
            raise SystemExit(f"{label}: could not clip an ear, so the fill is not a simple polygon")
    if len(ring) == 3 and len(set(ring)) == 3:
        triangles.append((ring[0], ring[1], ring[2]))
    return triangles


def inside_triangle(point, a, b, c) -> bool:
    """Whether `point` is inside a counter-clockwise triangle, edges included.

    Edges have to count. A diagonal that passes exactly through another vertex is
    the classic failure of strict ear clipping: the triangle looks empty, is
    clipped, and the vertex it grazed ends up in two triangles at once -- the
    triangulated area comes out larger than the polygon's. `entities.svg` has a
    plus whose diagonal does exactly that, which is why the test is closed.
    """
    return (cross3(a, b, point) >= -AREA_EPSILON
            and cross3(b, c, point) >= -AREA_EPSILON
            and cross3(c, a, point) >= -AREA_EPSILON)


@dataclass
class FillMesh:
    """One triangulated filled region, ready to become a `Shape::Mesh`."""

    points: list[tuple[float, float]]
    triangles: list[int]
    holes: int
    concave: bool
    outer_area: float
    hole_area: float
    mesh_area: float
    outer_triangles: int


def triangulate_fill(contours: list[list[tuple[float, float]]], even_odd: bool,
                     label: str) -> list[FillMesh]:
    """Every filled region of one SVG element, triangulated, holes subtracted.

    Returns one mesh per filled region. Disjoint contours of the same element stay
    separate meshes exactly as they were separate `PathShape`s before, while a
    contour nested as a hole is merged into the mesh of the contour that contains
    it -- that merge is the whole change.
    """
    for index in range(len(contours)):
        for other in range(index + 1, len(contours)):
            first, second = contours[index], contours[other]
            if any(
                segments_cross(first[edge], first[(edge + 1) % len(first)],
                               second[side], second[(side + 1) % len(second)])
                for edge in range(len(first))
                for side in range(len(second))
            ):
                raise SystemExit(
                    f"{label}: two filled contours cross; even-odd/nonzero fill of "
                    "a self-overlapping path needs a full tessellator, not ear clipping"
                )
    normalized = [counter_clockwise(contour) for contour in contours]
    # Grouping runs on the *original* windings: `nonzero` is defined by them, and
    # normalising every contour to counter-clockwise first would turn every
    # opposite-wound hole into a second solid blob. Only the clipping copy is
    # normalised.
    groups = fill_groups(contours, even_odd, label)
    concave = any(not fan_covers(contour) for contour in normalized)
    meshes: list[FillMesh] = []
    for group in groups:
        outer = normalized[group.outer]
        holes = [list(reversed(normalized[index])) for index in group.holes]
        # The vertex array is the outer contour followed by each hole; the bridge
        # ring below indexes into it, and repeats an index at each end of a
        # corridor rather than duplicating the coordinate.
        points: list[tuple[float, float]] = list(outer)
        hole_ids: list[list[int]] = []
        for hole in holes:
            start = len(points)
            points.extend(hole)
            hole_ids.append(list(range(start, start + len(hole))))
        ring = list(range(len(outer)))
        for hole_index, hole in enumerate(hole_ids):
            found = bridge(ring, hole, points, outer, holes)
            if found is None:
                raise SystemExit(f"{label}: no visible bridge from a hole to its outer contour")
            ring_position, hole_position = found
            rotated = hole[hole_position:] + hole[:hole_position + 1]
            ring = ring[:ring_position + 1] + rotated + [ring[ring_position]] + ring[ring_position + 1:]
        triangles = ear_clip(ring, points, label)
        for first, second, third in triangles:
            if triangle_area(points[first], points[second], points[third]) <= AREA_EPSILON:
                raise SystemExit(f"{label}: a zero-area triangle survived the ear clip")
            # Area equality alone would also pass a triangulation that covered a
            # piece of a hole and missed the same area elsewhere. The centre of
            # every triangle has to be filled ground: inside the outer contour and
            # outside every hole.
            centre = (
                (points[first][0] + points[second][0] + points[third][0]) / 3.0,
                (points[first][1] + points[second][1] + points[third][1]) / 3.0,
            )
            if not point_inside(centre, outer) or any(point_inside(centre, hole) for hole in holes):
                raise SystemExit(
                    f"{label}: triangle {first, second, third} covers ground the fill does not"
                )
        mesh_area = sum(triangle_area(points[a], points[b], points[c])
                        for a, b, c in triangles)
        outer_area = abs(signed_area(outer))
        hole_area = sum(abs(signed_area(hole)) for hole in holes)
        expected = outer_area - hole_area
        tolerance = max(1e-6, 1e-6 * outer_area)
        if abs(mesh_area - expected) > tolerance:
            raise SystemExit(
                f"{label}: triangulated {mesh_area:.6f} square units, but the outer "
                f"contour minus its holes is {expected:.6f} (outer {outer_area:.6f}, "
                f"{len(holes)} hole(s) {hole_area:.6f}, contour areas "
                f"{[round(abs(signed_area(contour)), 3) for contour in contours]}, "
                f"groups {[(group.outer, group.holes) for group in groups]})"
            )
        if holes and not mesh_area < outer_area:
            raise SystemExit(f"{label}: a hole was not subtracted from the fill")
        meshes.append(FillMesh(
            points=points,
            triangles=[index for triangle in triangles for index in triangle],
            holes=len(holes),
            concave=concave,
            outer_area=outer_area,
            hole_area=hole_area,
            mesh_area=mesh_area,
            outer_triangles=len(outer) - 2,
        ))
    return meshes


def fan_triangles(count: int) -> list[int]:
    """Triangles of a convex polygon, as a fan from its first vertex.

    Used for the discs that stand in for dots, round caps and round joins: they
    are regular polygons, so the fan that epaint would have drawn is exact for
    them and ear clipping would only be slower to say the same thing.
    """
    return [index for corner in range(1, count - 1)
            for index in (0, corner, corner + 1)]


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


def width_text(value: float) -> str:
    """A stroke width the way Rust's `Display` for `f32` spells it.

    `gen_icons.width_text` is the iced generator's answer to exactly this question
    -- it rounds to the emitted precision and then asks for the shortest spelling
    that round-trips through `f32` -- so the two files' exception lists are spelled
    the same way and the width test compares against the same strings.
    """
    return gen_icons.width_text(value)


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


def triangles_source(triangles: list[int]) -> list[str]:
    """The `&[...]` literal for one mesh's indices, twelve to a line.

    Empty for a stroke, which is most shapes, so the empty case is one line rather
    than an open-and-close pair: 1,734 strokes would otherwise each carry a
    two-line reminder that they are not meshes.
    """
    if not triangles:
        return ["        triangles: &[],"]
    per_line = 12
    out = ["        triangles: &["]
    for start in range(0, len(triangles), per_line):
        chunk = ", ".join(str(index) for index in triangles[start:start + per_line])
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

    out = output()
    iced_include = Path(os.path.relpath(ICED, out.parent)).as_posix()

    icons, sets, refused = collect()

    # Everything the file will hold, worked out once: the glyph's pieces, how many
    # of them, which filled elements needed triangulating, and the round pieces the
    # strokes asked for. `tally.gate()` is the refusal to emit an unrounded set.
    geometry: list[tuple] = []
    no_paint: list[str] = []
    dots: list[str] = []
    filled_elements = 0
    triangulated: list[tuple[str, int, Piece, bool]] = []
    tally = RoundTally()
    for name, rust, box, elements in icons:
        tolerance = max(FLATNESS_FRACTION * max(box), 1e-4)
        dot_limit = DOT_FRACTION * max(box)
        shapes: list[tuple[Piece, object]] = []
        for index, element in enumerate(elements):
            if element.commands and element.commands[0][0] == "NoPaint":
                no_paint.append(f"{name}#{index}")
                continue
            found = subpaths(element, tolerance, dot_limit, tally, f"{name}#{index}")
            if element.filled:
                filled_elements += 1
                for piece in found:
                    if piece.holes or piece.concave:
                        triangulated.append((name, index, piece, element.even_odd))
            for piece in found:
                if piece.dot:
                    dots.append(f"{name}#{index}")
                shapes.append((piece, element))
        if not shapes:
            raise SystemExit(f"{name}.svg: no shape egui could draw")
        geometry.append((name, rust, box, elements, shapes))
    tally.gate()

    shape_counts: list[tuple[str, str, int]] = [
        (name, rust, len(shapes)) for name, rust, _, _, shapes in geometry
    ]
    total_shapes = sum(count for _, _, count in shape_counts)
    distinct_counts = len({count for _, _, count in shape_counts})
    total_points = sum(len(piece.points) for _, _, _, _, shapes in geometry
                       for piece, _ in shapes)
    total_triangles = sum(len(piece.triangles) // 3 for _, _, _, _, shapes in geometry
                          for piece, _ in shapes)
    total_round_pieces = sum(piece.round for _, _, _, _, shapes in geometry
                             for piece, _ in shapes)
    total_caps = sum(piece.caps for _, _, _, _, shapes in geometry
                     for piece, _ in shapes)
    total_joins = total_round_pieces - total_caps
    round_subpaths = sum(1 for _, _, _, _, shapes in geometry
                         for piece, _ in shapes if piece.round)
    round_glyphs = len({name for name, _, _, _, shapes in geometry
                        for piece, _ in shapes if piece.round})
    filled_shapes = sum(1 for _, _, _, _, shapes in geometry
                        for piece, element in shapes if element.filled)
    mesh_shapes = sum(1 for _, _, _, _, shapes in geometry
                      for piece, _ in shapes if piece.triangles)
    total_holes = sum(piece.holes for _, _, _, _, shapes in geometry
                      for piece, _ in shapes)
    largest = max(shape_counts, key=lambda entry: entry[2])
    largest_mesh = max(
        ((name, sum(len(piece.triangles) // 3 for piece, _ in shapes))
         for name, _, _, _, shapes in geometry),
        key=lambda entry: entry[1],
    )
    width_exceptions = sorted({
        f"{name}:{width_text(element.stroke_width)}"
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
    # The filled elements that the old triangle-fan fill could not draw, in the
    # order the report prints them: per element, its triangles now against the
    # triangles its outer contour alone would take, and the area it should have.
    triangulated_elements: dict[tuple[str, int], dict[str, object]] = {}
    for name, index, piece, even_odd in triangulated:
        entry = triangulated_elements.setdefault(
            (name, index),
            {"triangles": 0, "outer": 0, "holes": 0, "mesh_area": 0.0,
             "outer_area": 0.0, "hole_area": 0.0, "even_odd": False},
        )
        entry["triangles"] += len(piece.triangles) // 3
        entry["outer"] += piece.outer_triangles
        entry["holes"] += piece.holes
        entry["mesh_area"] += piece.mesh_area
        entry["outer_area"] += piece.outer_area
        entry["hole_area"] += piece.hole_area
        entry["even_odd"] = entry["even_odd"] or even_odd
    triangulated_by_glyph: dict[str, dict[str, object]] = {}
    for (name, index), entry in triangulated_elements.items():
        glyph = triangulated_by_glyph.setdefault(
            name, {"triangles": 0, "outer": 0, "holes": 0, "elements": 0}
        )
        glyph["triangles"] += entry["triangles"]
        glyph["outer"] += entry["outer"]
        glyph["holes"] += entry["holes"]
        glyph["elements"] += 1

    numbers: dict[str, object] = {
        "glyphs": len(icons),
        "all": len(sets["ALL"]),
        "categories": len(sets["TAG_CATEGORIES"]),
        "loaders": len(sets["TAG_LOADERS"]),
        "shapes": total_shapes,
        "points": total_points,
        "triangles": total_triangles,
        "meshes": mesh_shapes,
        "fills": filled_shapes,
        "holes": total_holes,
        "joins": total_joins,
        "caps": total_caps,
        "round_pieces": total_round_pieces,
        "round_subpaths": round_subpaths,
        "round_glyphs": round_glyphs,
        "triangulated": len(triangulated_elements),
        "triangulated_glyphs": len(triangulated_by_glyph),
        "elements": sum(len(elements) for _, _, _, elements, _ in geometry),
        "no_paint": len(no_paint),
        "filled_elements": filled_elements,
        "filled_glyphs": len(filled_glyphs),
        "dots": len(dots),
        "dot_glyphs": len({entry.split('#')[0] for entry in dots}),
        "flatness": FLATNESS_FRACTION,
    }

    lines: list[str] = []
    add = lines.append

    add("//! The reference client's icon set, compiled into egui shapes.")
    add("//!")
    add("//! **Generated by `tools/gen_icons_egui.py`. Do not edit.**")
    add("//!")
    add(f"//! This is the egui twin of `{ICED_FROM_ROOT}`: the")
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
    add("//! **Shapes, not strings.** iced's path builder holds subpaths and cubic")
    add("//! Beziers; every epaint primitive holds finished points. So the generator")
    add("//! flattens every cubic at generation time -- recursive halving until both")
    add("//! control points are within {flatness} of the view box's long side".format(**numbers))
    add("//! of the chord, which is 0.05 px at 24 px and 0.1 px at 48 px, well")
    add("//! under epaint's half-pixel feathering -- and every coordinate in this file")
    add(f"//! is a plain number: {numbers['points']:,} of them, none computed twice.")
    add("//!")
    add("//! **The fills are triangulated, holes included.** `PathShape` has a fill")
    add("//! colour and no fill rule, and epaint tessellates it with a triangle fan from")
    add("//! its first point: exact only for a contour that is star-shaped from that")
    add("//! point, and with no way to leave a hole empty -- the fan paints a nested")
    add("//! contour solid. So every filled element is triangulated here, once, by ear")
    add("//! clipping with hole bridging, the construction SVG's fill rules are defined")
    add("//! against (the one mapbox's `earcut` implements). Each contour is wound the")
    add("//! same way and its nesting measured; the element's own `fill-rule` then")
    add("//! decides what is filled -- `evenodd` by nesting parity, `nonzero` by winding")
    add("//! number -- and each hole is cut into its outer contour by a bridge to the")
    add("//! nearest visible vertex. The corridor is walked twice, so it has no width")
    add("//! and adds no area: the triangles sum to the outer contour minus its holes.")
    add("//! The result is a `Shape::Mesh`, and a stroke stays a `PathShape`.")
    add(f"//! {numbers['meshes']:,} of the {numbers['shapes']:,} shapes are meshes: {numbers['fills']:,} filled,")
    add(f"//! {numbers['dots']:,} dots and {numbers['round_subpaths']:,} round-join meshes holding")
    add(f"//! {numbers['round_pieces']:,} discs and half-discs, with {numbers['triangles']:,} triangles in total.")
    add("//!")
    add("//! **Dots are discs.** {dots:,} stroked subpaths in the set are shorter than a".format(**numbers))
    add("//! thousandth of their view box -- the reference's dot idiom, a zero-length")
    add("//! line whose round cap is the whole of the picture (`M12 12h.01`) -- and")
    add("//! epaint has no round caps, so a stroke of one draws nothing at all. Each is")
    add("//! emitted as the disc that cap would have painted: a regular polygon of the")
    add("//! stroke's width, filled in the stroke's ink, marked `dot` in the tables.")
    add("//! {dot_glyphs} glyphs have at least one; a dot that vanished is the kind of".format(**numbers))
    add("//! difference nobody finds by reading coordinates.")
    add("//!")
    add(f"//! {numbers['triangulated']} of the {numbers['filled_elements']} filled elements, in {numbers['triangulated_glyphs']} glyphs,")
    add("//! could not have been drawn by that fan -- a concave outline, a hole, or")
    add("//! both. They are exact now, and this is what each one costs:")
    add("//!")
    for name, entry in sorted(triangulated_by_glyph.items()):
        holes = entry["holes"]
        cost = f"{entry['triangles']} triangles"
        if holes:
            cost += f" ({holes} hole{'s' if holes != 1 else ''})"
        add(f"//! * `{name}.svg` -- {cost}")
    add("//!")
    add("//! The iced side carries the reference's `fill-rule` and honours it, and so")
    add("//! does this file: the rule is read off the element and applied by the")
    add("//! triangulator rather than approximated away.")
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
    add("//! **Round joins and caps are discs.** `PathStroke` carries a width, a colour")
    add("//! and a `StrokeKind`, and no line cap or join; epaint miters every corner and")
    add("//! butts every line end. The reference declares")
    add("//! `stroke-linecap=\"round\"`/`stroke-linejoin=\"round\"` on 413 of the 441")
    add("//! glyphs, so every stroked subpath that turns or ends is followed by one")
    add("//! small mesh of round pieces -- the same no-attribute problem as the holes,")
    add("//! and the second entry in that parity-log paragraph:")
    add("//!")
    add(f"//! * {numbers['round_pieces']:,} discs at {numbers['joins']:,} corners whose turn exceeds ~20 degrees,")
    add(f"//!   and {numbers['caps']:,} half-discs at open ends, over {numbers['round_subpaths']:,} stroked subpaths")
    add(f"//!   in {numbers['round_glyphs']} glyphs. A closed subpath takes joins but no caps.")
    add("//!")
    add("//! A disc of the stroke's radius, centred on the vertex, is the arc SVG would")
    add("//! have painted there; it is drawn over epaint's miter, not instead of it.")
    add("//! What is left is the miter's tip where a corner is sharp enough to push it")
    add("//! past the disc (a right angle puts it 41% of the radius beyond): the path")
    add("//! has already painted that triangle before the disc goes over it, and a")
    add("//! shape cannot erase what it did not paint.")
    add("//!")
    add("//! ## Performance")
    add("//!")
    add("//! These shapes are drawn every frame a window draws, so the cost model matters")
    add("//! more than the last decimal of a coordinate.")
    add("//!")
    add("//! * **The geometry is `&'static`.** No SVG is parsed at runtime, no path is")
    add("//!   built from a string, and the coordinates and triangle indices live in")
    add("//!   the binary. Drawing a glyph allocates one `Vec<Shape>`, one `Vec<Pos2>`")
    add("//!   per stroke and one `Mesh` per filled or round piece -- each pre-sized")
    add("//!   with `Vec::with_capacity` -- and nothing else.")
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
    add("//! The meshes here are built per draw from view-box coordinates, not baked")
    add("//! into absolute positions. Building a mesh skips epaint's tessellation, but")
    add("//! egui bakes *absolute* positions into one and has no per-shape transform at")
    add("//! paint time, so a cache of baked meshes would have to include the widget's")
    add("//! position in its key. A scrolling list moves its icons every frame; every")
    add("//! one of them would miss, rebuild its mesh per icon per frame, and the")
    add("//! bounded cache would clear itself continuously. Building in view-box")
    add("//! coordinates keeps the key the glyph, the fit and the ink -- the same three")
    add("//! things a path shape ever needed.")
    add("")
    add("// Nothing draws these yet: `crates/sift-ui` is the first slice of the")
    add("// move from iced to egui (see its `Cargo.toml`), and it is a frame-time")
    add("// harness rather than the launcher. Delete this line the day the last icon is")
    add("// drawn from here.")
    add("#![allow(dead_code)]")
    add("")
    add("use std::cell::RefCell;")
    add("")
    add("use egui::epaint::{Mesh, PathShape, PathStroke, Vertex};")
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
    add("/// One drawable piece of an icon: points and how to paint them.")
    add("///")
    add("/// This is one *shape*, not one SVG element. An element with three stroked")
    add("/// subpaths is three of these, because a `PathShape` cannot hold two; a filled")
    add("/// element is one per filled region, because its contours are triangulated")
    add("/// together so a hole stays empty -- see the module documentation. The iced")
    add("/// file's `Element` is the element-level record; this is the shape-level one.")
    add("#[derive(Clone, Copy, Debug, PartialEq)]")
    add("pub struct Subpath {")
    add("    /// The polyline, in the icon's own view box, as x/y pairs.")
    add("    ///")
    add("    /// For a filled subpath these are the mesh's vertices: the outer contour")
    add("    /// followed by each hole, with a bridge repeated rather than duplicated.")
    add("    pub points: &'static [(f32, f32)],")
    add("    /// For a filled subpath, the triangles of `points`, three indices each.")
    add("    ///")
    add("    /// Empty for a stroke. The indices are why a fill is a `Shape::Mesh` and")
    add("    /// not a `PathShape`: they are the holes and concavities epaint's triangle")
    add("    /// fan could not express.")
    add("    pub triangles: &'static [u32],")
    add("    /// Whether it closes back to its first point. True for every filled one.")
    add("    pub closed: bool,")
    add("    /// Stroke width in view-box units, before the fit scale is applied.")
    add("    pub stroke_width: f32,")
    add("    /// Whether this is filled rather than stroked.")
    add("    pub filled: bool,")
    add("    /// Whether this is one of the reference's dots: a zero-length stroke whose")
    add("    /// round cap was the whole picture, emitted as the disc that cap painted.")
    add("    ///")
    add("    /// Every dot is `filled` and `closed`, and its points are a regular polygon")
    add("    /// around the cap's centre. It exists as a field rather than as a quiet")
    add("    /// property of the points so a reader can tell a converted dot from a fill.")
    add("    pub dot: bool,")
    add("    /// How many round joins and caps this mesh holds, or 0.")
    add("    ///")
    add("    /// A round-join mesh is `filled` and `closed`, always follows the stroked")
    add("    /// subpath it belongs to, and exists because neither `PathStroke` nor")
    add("    /// epaint's tessellator has a cap or join to set. Like `dot`, it is a field")
    add("    /// so a reader and a test can tell it from a filled glyph.")
    add("    pub round: u32,")
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
    add("/// One `Shape` per [`Subpath`], and no more: the vector is pre-sized, each")
    add("/// stroke's points are written into a vector sized for them, and each mesh's")
    add("/// vertices straight into the mesh's own.")
    add("pub fn parts(glyph: Glyph, fit: Fit, ink: Color32) -> Vec<Shape> {")
    add("    let subpaths = glyph.subpaths();")
    add("    let mut shapes = Vec::with_capacity(subpaths.len());")
    add("    for subpath in subpaths {")
    add("        let resolved = subpath.ink.resolve(ink);")
    add("        let color = if subpath.opacity >= 1.0 {")
    add("            resolved")
    add("        } else {")
    add("            resolved.gamma_multiply(subpath.opacity)")
    add("        };")
    add("        if subpath.filled {")
    add("            // A fill is a triangle list, not a path: the triangles are the")
    add("            // holes and concavities a `PathShape`'s fan would have got wrong.")
    add("            // A mesh has no stroke, so epaint does not also outline the fill.")
    add("            let mut mesh = Mesh::default();")
    add("            mesh.vertices.reserve(subpath.points.len());")
    add("            for &(x, y) in subpath.points {")
    add("                mesh.vertices.push(Vertex::untextured(")
    add("                    Pos2::new(")
    add("                        x.mul_add(fit.scale, fit.offset.x),")
    add("                        y.mul_add(fit.scale, fit.offset.y),")
    add("                    ),")
    add("                    color,")
    add("                ));")
    add("            }")
    add("            mesh.indices.reserve(subpath.triangles.len());")
    add("            mesh.indices.extend_from_slice(subpath.triangles);")
    add("            shapes.push(Shape::mesh(mesh));")
    add("            continue;")
    add("        }")
    add("        let mut points = Vec::with_capacity(subpath.points.len());")
    add("        points.extend(subpath.points.iter().map(|&(x, y)| {")
    add("            Pos2::new(x.mul_add(fit.scale, fit.offset.x), y.mul_add(fit.scale, fit.offset.y))")
    add("        }));")
    add("        let path = if subpath.closed {")
    add("            PathShape::closed_line(")
    add("                points,")
    add("                PathStroke::new(subpath.stroke_width * fit.scale, color),")
    add("            )")
    add("        } else {")
    add("            PathShape::line(points, PathStroke::new(subpath.stroke_width * fit.scale, color))")
    add("        };")
    add("        shapes.push(Shape::Path(path));")
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
    add("#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]")
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
        for piece, element in shapes:
            if piece.dot:
                kind = "dot (a round-capped zero-length stroke, as a disc)"
            elif piece.round:
                kind = (f"round joins and caps as {piece.round} disc(s), "
                        f"{piece.caps} of them caps")
            elif element.filled:
                kind = "filled mesh"
            else:
                kind = "stroked"
            add(f"    // {kind} at {number(element.stroke_width, WIDTH_DECIMALS)}, "
                f"{len(piece.points)} point(s)")
            if piece.triangles:
                detail = f"{len(piece.triangles) // 3} triangle(s)"
                if piece.holes:
                    detail += f", {piece.holes} hole(s)"
                add(f"    // {detail}")
            add("    Subpath {")
            lines.extend(points_source(piece.points))
            lines.extend(triangles_source(piece.triangles))
            add(f"        closed: {'true' if piece.closed else 'false'},")
            add(f"        stroke_width: {number(element.stroke_width, WIDTH_DECIMALS)},")
            add(f"        filled: {'true' if element.filled or piece.dot or piece.round else 'false'},")
            add(f"        dot: {'true' if piece.dot else 'false'},")
            add(f"        round: {piece.round},")
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
    add(f"        let iced = include_str!(\"{iced_include}\");")
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
    add("                    Shape::Mesh(mesh) => {")
    add("                        // A filled or round-join piece: a real triangle list,")
    add("                        // every index in range, every vertex finite, and")
    add("                        // every vertex painting a non-empty colour.")
    add("                        assert!(mesh.vertices.len() >= 3, \"{name} has a mesh with no triangle\");")
    add("                        assert!(mesh.indices.len() >= 3, \"{name} has a mesh with no triangle\");")
    add("                        assert_eq!(mesh.indices.len() % 3, 0, \"{name} has a partial triangle\");")
    add("                        for index in &mesh.indices {")
    add("                            assert!(")
    add("                                (*index as usize) < mesh.vertices.len(),")
    add("                                \"{name} has a mesh index past its vertices\"")
    add("                            );")
    add("                        }")
    add("                        for vertex in &mesh.vertices {")
    add("                            assert!(")
    add("                                vertex.pos.x.is_finite() && vertex.pos.y.is_finite(),")
    add("                                \"{name} has a vertex that is not a number\"")
    add("                            );")
    add("                            assert!(")
    add("                                vertex.color != Color32::TRANSPARENT,")
    add("                                \"{name} has a vertex that paints nothing\"")
    add("                            );")
    add("                        }")
    add("                    }")
    add("                    other => panic!(")
    add("                        \"{name} produced something that is neither a path nor a mesh: {other:?}\"")
    add("                    ),")
    add("                }")
    add("            }")
    add("        }")
    add("    }")
    add("")
    add("    /// The recorded shape count for every glyph, in the order the enum has them.")
    add("    ///")
    add("    /// **This table is a change detector, not a truth.** Nothing in the world")
    add("    /// says `Glyph::Home` has two shapes; what the table says is that it had two")
    add("    /// when this file was generated. A change to the flattening tolerance, the")
    add("    /// join threshold, the subpath splitting, or a vendored SVG then fails here,")
    add("    /// with the glyph named, rather than showing up as a picture that is subtly")
    add("    /// different in a window nobody happens to be comparing.")
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
    add(f"        assert_eq!(distinct.len(), {distinct_counts}, \"the counts have collapsed or spread\");")
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
    add("        let mut output = ctx.run_ui(egui::RawInput::default(), |ui| {")
    add("            for (_, glyph) in every() {")
    add("                draw(ui.painter(), glyph, empty, Color32::WHITE);")
    add("                let _ = cached(ui.painter(), glyph, empty, Color32::WHITE);")
    add("                draw(ui.painter(), glyph, inverted, Color32::WHITE);")
    add("            }")
    add("        });")
    add("        // A font atlas delta is produced for the text this frame laid out, and")
    add("        // epaint panics if one is dropped without being uploaded. Nothing uploads")
    add("        // in a unit test, so the delta is cleared deliberately -- which is what the")
    add("        // panic message asks for.")
    add("        output.textures_delta.clear();")
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
    add("    fn the_fills_are_closed_triangulated_meshes_with_no_stroke() {")
    add("        // A filled subpath is a `Shape::Mesh`: closed, filled, with a")
    add("        // triangle list that indexes its own points and no stroke, so")
    add("        // epaint does not also outline it. A fill that arrived open, or")
    add("        // without its triangles, would be the wrong primitive rather than")
    add("        // the wrong geometry -- the kind of thing that renders and looks")
    add("        // nearly right.")
    add("        let mut filled = 0;")
    add("        for (name, glyph) in every() {")
    add("            for subpath in glyph.subpaths() {")
    add("                if !subpath.filled || subpath.dot || subpath.round > 0 {")
    add("                    continue;")
    add("                }")
    add("                filled += 1;")
    add("                assert!(subpath.closed, \"{name} has an open fill\");")
    add("                assert!(subpath.points.len() >= 3, \"{name} has a fill with no area\");")
    add("                assert!(!subpath.triangles.is_empty(), \"{name} has a fill with no triangles\");")
    add("                assert_eq!(")
    add("                    subpath.triangles.len() % 3,")
    add("                    0,")
    add("                    \"{name} has a fill with a partial triangle\"")
    add("                );")
    add("                for index in subpath.triangles {")
    add("                    assert!(")
    add("                        (*index as usize) < subpath.points.len(),")
    add("                        \"{name} has a fill index past its points\"")
    add("                    );")
    add("                }")
    add("            }")
    add("        }")
    add(f"        assert_eq!(filled, {filled_shapes}, \"the filled shape count moved\");")
    add("    }")
    add("")
    add("    #[test]")
    add("    fn the_round_joins_follow_their_strokes_and_are_triangulated() {")
    add("        // epaint has no round cap or join, so a stroked subpath that turns")
    add("        // or ends is followed by one small mesh of discs and half-discs.")
    add("        // This is the gate on that: every such mesh is filled, closed, not a")
    add("        // dot, has triangles, and comes directly after the stroke it rounds.")
    add("        // The generator's `RoundTally` refuses a run that produces none at")
    add("        // all; this catches a file where they were emitted somewhere else,")
    add("        // or quietly dropped on the way out.")
    add("        let mut discs = 0u32;")
    add("        let mut meshes = 0;")
    add("        for (name, glyph) in every() {")
    add("            let subpaths = glyph.subpaths();")
    add("            for (index, subpath) in subpaths.iter().enumerate() {")
    add("                if subpath.round == 0 {")
    add("                    continue;")
    add("                }")
    add("                meshes += 1;")
    add("                discs += subpath.round;")
    add("                assert!(subpath.filled && subpath.closed, \"{name} has an open round mesh\");")
    add("                assert!(!subpath.dot, \"{name} has a round mesh marked as a dot\");")
    add("                assert!(index > 0, \"{name} has a round mesh before any stroke\");")
    add("                let stroke = &subpaths[index - 1];")
    add("                assert!(")
    add("                    !stroke.filled && !stroke.dot,")
    add("                    \"{name} rounds something that is not a stroke\"")
    add("                );")
    add("                assert!(")
    add("                    subpath.triangles.len() >= 3 * subpath.round as usize,")
    add("                    \"{name} has fewer triangles than round pieces\"")
    add("                );")
    add("            }")
    add("        }")
    add(f"        assert_eq!(meshes, {round_subpaths}, \"the number of round meshes moved\");")
    add(f"        assert_eq!(discs, {total_round_pieces}, \"the number of round joins and caps moved\");")
    add("    }")
    add("")
    add("    #[test]")
    add("    fn the_dots_the_reference_draws_are_drawn_as_discs() {")
    add("        // The set draws a dot as a zero-length line with a round cap --")
    add("        // `M12 12h.01` -- and the cap is the whole of what you see. epaint has")
    add("        // no round caps, so a stroke of one draws nothing at all, and these")
    add("        // would be dots that quietly vanished. `tools/gen_icons_egui.py`")
    add("        // converts each into the disc the cap would have painted, and this is")
    add("        // the gate on that conversion: every dot is a closed, filled polygon")
    add("        // whose vertices sit on the circle of the stroke's radius.")
    add("        let mut dots = 0;")
    add("        let mut glyphs = 0;")
    add("        for (name, glyph) in every() {")
    add("            let mut has_dot = false;")
    add("            for subpath in glyph.subpaths() {")
    add("                if !subpath.dot {")
    add("                    continue;")
    add("                }")
    add("                dots += 1;")
    add("                has_dot = true;")
    add("                assert!(subpath.filled && subpath.closed, \"{name} has an open dot\");")
    add("                assert!(subpath.points.len() >= 12, \"{name} has a dot that is not round\");")
    add("                let count = subpath.points.len() as f32;")
    add("                let middle = subpath.points.iter().fold((0.0f32, 0.0f32), |sum, point| {")
    add("                    (sum.0 + point.0, sum.1 + point.1)")
    add("                });")
    add("                let middle = (middle.0 / count, middle.1 / count);")
    add("                let radius = subpath.stroke_width * 0.5;")
    add("                for point in subpath.points {")
    add("                    let distance = ((point.0 - middle.0).powi(2) + (point.1 - middle.1).powi(2)).sqrt();")
    add("                    assert!(")
    add("                        (distance - radius).abs() <= 0.01,")
    add("                        \"{name} has a dot point {distance} from the centre, not {radius}\"")
    add("                    );")
    add("                }")
    add("            }")
    add("            if has_dot {")
    add("                glyphs += 1;")
    add("            }")
    add("        }")
    add(f"        assert_eq!(dots, {len(dots)}, \"the number of dots moved\");")
    add(f"        assert_eq!(glyphs, {len({entry.split('#')[0] for entry in dots})}, \"the number of glyphs with a dot moved\");")
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
    add("        assert!(!fixed.is_empty(), \"the list is empty, so this proves nothing\");")
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
    add("        assert!(!tokens.is_empty(), \"the list is empty, so this proves nothing\");")
    add("        assert_eq!(tokens, expected, \"the tokens the set asks for moved\");")
    add("    }")
    add("}")
    add("")

    text = "\n".join(lines)

    # The per-element triangle counts and area check for the glyphs that needed
    # triangulation: the evidence that a hole is a second ring of triangles and not
    # a solid blob. `triangulate_fill` already refuses a mesh whose area is not
    # outer-minus-holes; this prints the arithmetic for a reader.
    # The filled triangles a glyph draws, which for most of the problem glyphs is
    # the element that needed triangulating but for a few (a ring plus a separate
    # disc, a potato plus its eyes) includes simple fills as well.
    glyph_fill_triangles: dict[str, int] = {}
    for name, _, _, _, shapes in geometry:
        glyph_fill_triangles[name] = sum(
            len(piece.triangles) // 3 for piece, element in shapes if element.filled
        )
    triangulated_lines: list[str] = []
    area_lines: list[str] = []
    for (name, index), entry in sorted(triangulated_elements.items()):
        holes = entry["holes"]
        detail = f"{entry['triangles']} triangles, outer alone {entry['outer']}"
        if holes:
            detail += f", {holes} hole{'s' if holes != 1 else ''}"
        if entry["even_odd"]:
            detail += ", fill-rule evenodd"
        if glyph_fill_triangles[name] != entry["triangles"]:
            detail += f"; all fills in the glyph {glyph_fill_triangles[name]}"
        triangulated_lines.append(f"  {name} element {index + 1}: {detail}")
        if not holes:
            continue
        expected = entry["outer_area"] - entry["hole_area"]
        delta = entry["mesh_area"] - expected
        passed = entry["mesh_area"] < entry["outer_area"]
        area_lines.append(
            f"  {name}#{index + 1}: mesh {entry['mesh_area']:.3f} < outer "
            f"{entry['outer_area']:.3f} (holes {entry['hole_area']:.3f}, expected "
            f"{expected:.3f}, delta {delta:+.6f}) {'OK' if passed else 'FAILED'}"
        )
        if not passed:
            raise SystemExit(f"{name}#{index + 1}: the hole was not subtracted from the fill")
    glyph_triangle_totals = ", ".join(
        f"{name} {entry['triangles']}"
        for name, entry in sorted(triangulated_by_glyph.items())
    )

    report = [
        f"glyphs       {len(icons)}",
        "  " + "  ".join(
            f"{source['table']} {len(sets[source['table']])}" for source in gen_icons.SOURCES
        ),
        f"shapes       {total_shapes}",
        f"points       {total_points}",
        f"triangles    {total_triangles}",
        f"meshes       {mesh_shapes}: {filled_shapes} filled, {numbers['dots']} dots, "
        f"{round_subpaths} round-join",
        f"holes        {total_holes} across {numbers['triangulated']} triangulated elements",
        f"round        {total_joins} join disc(s) + {total_caps} half-disc cap(s) over "
        f"{round_subpaths} stroked subpath(s) in {round_glyphs} glyph(s)",
        f"elements     {numbers['elements']} in total, {numbers['no_paint']} paint nothing",
        f"largest      {largest[0]} ({largest[2]} shapes); "
        f"most triangles {largest_mesh[0]} ({largest_mesh[1]} triangles)",
        f"filled       {numbers['filled_elements']} elements over {numbers['filled_glyphs']} glyphs",
        f"even-odd     {len(even_odds)}: {', '.join(even_odds) if even_odds else '-'}",
        f"triangulated {numbers['triangulated']} elements over {numbers['triangulated_glyphs']} glyphs: "
        + glyph_triangle_totals,
        *triangulated_lines,
        "area check   outer contour minus holes, per holed element:",
        *area_lines,
        f"no paint     {len(no_paint)}: {', '.join(sorted(no_paint)) if no_paint else '-'}",
        f"dots         {len(dots)} over {len({entry.split('#')[0] for entry in dots})} glyphs: "
        + (", ".join(sorted({entry.split('#')[0] for entry in dots})) if dots else "-"),
        f"refused      {len(refused)}: {', '.join(name for name, _ in sorted(refused)) or '-'}",
        f"off-width    {len(width_exceptions)}: {', '.join(width_exceptions) if width_exceptions else '-'}",
        f"tokens       {', '.join(tokens) if tokens else '-'}",
        f"fixed ink    {', '.join(fixed_colours) if fixed_colours else '-'}",
        f"fill glyphs  {', '.join(filled_glyphs) if filled_glyphs else '-'}",
    ]

    if args.check:
        current = out.read_text(encoding="utf-8") if out.is_file() else ""
        if current != text:
            print(f"{out.relative_to(ROOT)} is not what this tool emits.", file=sys.stderr)
            print("Run `python tools/gen_icons_egui.py` and commit the result.", file=sys.stderr)
            return 1
        print("egui icon generation is byte-identical")
        return 0

    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(text, encoding="utf-8", newline="\n")
    if args.report:
        for line in report:
            print(line)
    print(f"wrote {out.relative_to(ROOT)} "
          f"({text.count(chr(10)) + 1} lines, {len(text.encode('utf-8'))} bytes)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
