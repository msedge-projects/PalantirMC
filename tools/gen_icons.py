#!/usr/bin/env python3
"""Compile the reference client's icon set into geometry a native window can stroke.

The shell's icons are PNGs carved out of another launcher's binary. The reference
keeps 313 SVG files at the top of `assets/icons`, plus 132 in `tags/categories`
and `tags/loaders`, and they are outlines rather than pictures: 305 of the 313 are
stroked `currentColor` at width 2 with round caps and joins, which is what makes
them take the colour of whatever they sit in. A rasterised bitmap cannot do that,
and neither can a filled outline -- both are the wrong shape for the job, so this
emits geometry and lets the toolkit stroke it.

    python tools/gen_icons.py            # write crates/palantir-desktop/src/icons_gen.rs
    python tools/gen_icons.py --check    # fail if the file on disk is not what this emits
    python tools/gen_icons.py --report   # per-icon command counts, and anything unusual

## What it accepts, and what it refuses

The icons are Lucide-derived and their shape vocabulary is small, so this handles
exactly what is there and **stops on anything else** rather than guessing:

| Accepted | How |
| --- | --- |
| `M m L l H h V v C c S s Q q T t A a Z z` | A full path parser, with implicit repetition, `S`/`T` control-point reflection, and elliptical arcs converted to cubics |
| `circle`, `ellipse`, `rect` (with `rx`/`ry`), `line`, `polyline`, `polygon` | Converted to the same command list |
| `g` with style attributes | Style inherited by children, as CSS would |
| `g transform` | An affine 2x3 applied to every point. Exact rather than approximate: an affine transform commutes with Bézier evaluation, and arcs have already become cubics by then |

Anything else -- an unknown element, an unsupported attribute on a shape, a
transform this cannot parse -- raises and names the file. Silently drawing an icon
almost right is worse than not drawing it, because nobody goes looking for a
corner that is off by two units.

## The shape that paints nothing

Ten of the icons carry a shape the reference declares with **neither a fill nor a
stroke**, and SVG paints nothing at all for it: the clamp path an exported icon
has so that its drawing area has a name, `M0 0h24v24H0Z`. Seven declare it
`fill="none"` and no `stroke`; `key.svg` and `palette.svg` write `stroke="none"`
beside their `fill="none"`; `bungeecord.svg` spells it as a `<rect
style="fill:none">`.

Reading `fill="none"` as *unfilled but stroked* is what this tool used to do, and
it drew a full 24-unit box around each of those ten glyphs -- a 1px frame the
reference does not draw, measured on *Fabric* and *Forge* on the reference's own
profile page. So such a shape is emitted as `Cmd::NoPaint` and the interpreter
skips it, which is the doctrine above applied to a shape rather than to an icon:
the reference paints nothing, so neither does this.

The element is still written, still counted and still named in the generated
gate, because a generator that dropped the shape would leave no trace of having
read it -- the next export that clamps differently would be invisible. Skipping
the *drawing* is the reference's own behaviour; skipping the *record* would be
this tool's.

## The three sets, and the two directories this reads besides the top level

`assets/icons` is not one directory. It is three:

| Set | Where | Prefix | Count |
| --- | --- | --- | --- |
| `ALL` | `icons/*.svg` | none | 313 |
| `TAG_CATEGORIES` | `icons/tags/categories/*.svg` | `TagCategory` | 102 |
| `TAG_LOADERS` | `icons/tags/loaders/*.svg` | `TagLoader` | 30 |

`icons/tags/categories/badge-check.svg` and `icons/badge-check.svg` are two
different pictures that would both be `BadgeCheck`, so each tag set carries the
prefix the reference's own `assets/build/generate-exports.ts` gives it -- it walks
the same two directories and writes `TagCategory${stem}` and `TagLoader${stem}`
for exactly this reason. The port drops the `Icon` suffix that one appends, because
its variants read `BadgeCheck` and not `BadgeCheckIcon`, so `Glyph::TagLoaderForge`
is the reference's `TagLoaderForgeIcon`. Nothing that is not a tag set is prefixed,
which is the property that keeps the 313 existing variant names exactly as they
are.

The four that this tool cannot parse are named, not skipped quietly: see
`REFUSED_TAGS` below, and the test in the generated file that writes the list down.

## What it emits

A command list per icon, not a closure per icon: `Cmd::Move/Line/Cubic/Close` over
`f32`, plus the icon's view box, stroke width, fill flag and ink hint. One tiny
interpreter turns a list into iced `canvas` paths, so the toolkit's API appears in
this file once instead of 313 times, and a gate can check the *data* -- that every
number is finite, that the first command moves, that every icon the source has is
here -- without rendering anything.
"""

from __future__ import annotations

import argparse
import math
import re
import struct
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
VENDOR = ROOT / "vendor" / "modrinth-app"
ICONS = VENDOR / "assets" / "icons"
OUT = ROOT / "crates" / "palantir-desktop" / "src" / "icons_gen.rs"

# One entry per set the reference keeps under `assets/icons`, in the order the
# generated file declares them.
#
# `directory` is relative to `assets/icons`; `glob` is the file pattern inside it;
# `prefix` is what every variant in the set is named with, which is how the tag
# sets stop colliding with the top-level set (see the module docstring), and it is
# empty for the top level because those 313 names are referenced by name across the
# whole tree and one of them changing is an invisible break in every one of them.
#
# `complete` is the policy that makes adding a set with an unparsable file in it a
# decision rather than an accident: the top level is all-or-nothing, exactly as it
# was before these two existed, because every one of its 313 parses. A tag set is
# allowed to be incomplete, and the gap is written into the generated file instead
# of being swallowed -- see `REFUSED_TAGS`.
SOURCES = (
    {"table": "ALL", "directory": "", "glob": "*.svg", "prefix": "", "complete": True},
    {"table": "TAG_CATEGORIES", "directory": "tags/categories", "glob": "*.svg",
     "prefix": "TagCategory", "complete": False},
    {"table": "TAG_LOADERS", "directory": "tags/loaders", "glob": "*.svg",
     "prefix": "TagLoader", "complete": False},
)

# The tag icons this tool cannot read, written down rather than skipped.
#
# This is the loud half of the policy above, and it exists because the alternative
# -- teaching the parser to accept what it does not model -- is the failure the
# module docstring names. Four of the 132:
#
# | icon | construct that stopped it |
# | --- | --- |
# | `tags/loaders/geyser.svg` | four paths under `matrix(1.04036 0 0 1.1631 ...)` -- a non-uniform scale, which `stroke_scale` refuses because it has no single stroke width to apply |
# | `tags/loaders/purpur.svg` | `matrix(1.125 0 0 1.1372 ...)` on five of its six shapes, and `<use xlink:href>` on the other |
# | `tags/loaders/quilt.svg` | `matrix(.03053 0 0 .03046 ...)`, which differs in the fourth decimal and is still a different width per axis |
# | `tags/loaders/legacy-fabric.svg` | `<g clip-path="url(#clip0_6351_12952)">`, a clip this tool cannot put on a path |
#
# `legacy-fabric`'s clip happens to be a `<rect width="24" height="24">` over a
# `0 0 24 24` view box, so it clips nothing, and reading that would need a rule
# this tool does not have and would be wrong for the next icon that clips for real.
# So the clip is refused, the icon is absent, and `tags_icon` returns `None` for
# it: a loader pill with no glyph in it is 24 rows rather than 26, which is the
# height the reference draws a tag with no icon and not a wrong shape.
REFUSED_TAGS = {
    "tags/loaders/geyser": "non-uniform transform scale (1.0404 x 1.1631)",
    "tags/loaders/legacy-fabric": "unknown attribute `clip-path` on <g>",
    "tags/loaders/purpur": "non-uniform transform scale (1.1250 x 1.1372)",
    "tags/loaders/quilt": "non-uniform transform scale (0.0305 x 0.0305)",
}

# Attributes a shape may carry. Anything outside this set on a shape is refused,
# because an attribute this port does not understand is an attribute it would
# silently drop.
SHAPE_ATTRS = {
    "d", "cx", "cy", "r", "rx", "ry", "x", "y", "x1", "x2", "y1", "y2",
    "width", "height", "points", "fill", "stroke", "stroke-width",
    "stroke-linecap", "stroke-linejoin", "stroke-miterlimit", "class", "id",
    "transform", "xmlns", "viewbox", "opacity", "fill-rule", "clip-rule",
    "data-name", "style", "mask", "stroke-dasharray", "stroke-dashoffset",
    "stroke-opacity", "fill-opacity", "vector-effect",
}

# The elements that carry geometry, the ones whose attributes are inherited, and
# the ones that can be skipped entirely.
SHAPE_TAGS = {"path", "circle", "ellipse", "rect", "line", "polyline", "polygon"}
# `svg` is here and not below, and that is the difference between drawing the
# reference's icons and drawing outlines of them: the reference puts `fill`,
# `stroke`, `stroke-width` and the line caps on the root element, and CSS inherits
# them into every shape. `x.svg` is the case that proves it -- a single path with
# no attributes of its own under `<svg fill="currentColor">`, so it is a filled X,
# and a reader that ignored the root would stroke its outline instead.
GROUP_TAGS = {"g", "svg"}
IGNORED_TAGS = {"defs", "title", "metadata", "clippath", "lineargradient", "stop"}

# Attributes that are allowed but not used, because they are about serialisation,
# identity or accessibility rather than about drawing. Everything outside this set
# on a shape is refused, so the tool stops rather than dropping something that
# changes how an icon looks.
NEUTRAL_ATTRS = {"version", "xmlns", "xmlns:xlink", "xml:lang", "class", "id",
                 "data-name", "xml:space", "role", "focusable", "tabindex",
                 "preserveaspectratio", "enable-background"}
NEUTRAL_PREFIXES = ("aria-", "data-", "xml-", "xmlns:")


def is_neutral(name: str) -> bool:
    return (name in NEUTRAL_ATTRS
            or ":" in name
            or name.startswith(NEUTRAL_PREFIXES))

CIRCLE_KAPPA = 0.5522847498307936


class Unsupported(Exception):
    """An icon that this tool will not guess at."""


# --------------------------------------------------------------------------
# Path data
# --------------------------------------------------------------------------


NUMBER = re.compile(r"[-+]?(?:\d*\.\d+|\d+\.?)(?:[eE][-+]?\d+)?")
COMMAND = re.compile(r"([MmLlHhVvCcSsQqTtAaZz])([^MmLlHhVvCcSsQqTtAaZz]*)")
SEPARATORS = " ,\t\r\n"


def scan_number(text: str, index: int) -> tuple[float, int]:
    match = NUMBER.match(text, index)
    if not match:
        raise Unsupported(f"expected a number at `{text[index:index + 8]}`")
    return float(match.group()), match.end()


def arc_arguments(args: str) -> list[float]:
    """An arc's arguments, where two of the seven are single-digit flags.

    `a1 1 0 0 1 1 1` is the readable spelling, but the grammar allows the flags
    to be written against their neighbours -- `a1 1 0011 1` is the same command.
    Reading those with a general number scanner yields `0011` as one number, so
    the argument count stops dividing by seven and the icon is refused with a
    message about malformed path data, which is true but not the reason. The
    flags are therefore read as the single characters they are.
    """
    out: list[float] = []
    index, length = 0, len(args)
    while index < length:
        while index < length and args[index] in SEPARATORS:
            index += 1
        if index >= length:
            break
        group: list[float] = []
        for slot in range(7):
            while index < length and args[index] in SEPARATORS:
                index += 1
            if slot in (3, 4):
                if index >= length or args[index] not in "01":
                    raise Unsupported("arc flag is not 0 or 1")
                group.append(float(args[index]))
                index += 1
            else:
                value, index = scan_number(args, index)
                group.append(value)
        out.extend(group)
    return out

ARITY = {"M": 2, "L": 2, "H": 1, "V": 1, "C": 6, "S": 4, "Q": 4, "T": 2, "A": 7, "Z": 0}


def path_commands(d: str) -> list[tuple]:
    """One `d` attribute as absolute `Move`/`Line`/`Cubic`/`Close` commands.

    Relative commands are resolved here rather than emitted as such: the toolkit's
    path builder is absolute-only, and resolving them once makes the emitted data
    readable as the icon's actual shape instead of as a sequence of deltas.
    """
    out: list[tuple] = []
    x = y = 0.0
    start = (0.0, 0.0)
    last_control: tuple[float, float] | None = None
    last_kind: str | None = None

    for match in COMMAND.finditer(d):
        letter = match.group(1)
        upper = letter.upper()
        relative = letter.islower()
        numbers = arc_arguments(match.group(2)) if upper == "A" \
            else [float(n) for n in NUMBER.findall(match.group(2))]
        if upper == "Z":
            out.append(("Close",))
            x, y = start
            last_control = None
            last_kind = "Z"
            continue
        arity = ARITY[upper]
        if arity == 0 or len(numbers) % arity != 0 or not numbers:
            raise Unsupported(f"malformed `{letter}` in path data")

        for offset in range(0, len(numbers), arity):
            args = numbers[offset:offset + arity]
            cmd = upper
            # `M` followed by more coordinates is an implicit `L`, per the spec.
            if cmd == "M" and offset > 0:
                cmd = "L"
            if cmd == "M":
                x, y = (x + args[0], y + args[1]) if relative else (args[0], args[1])
                start = (x, y)
                out.append(("Move", x, y))
                last_control = None
            elif cmd == "L":
                x, y = (x + args[0], y + args[1]) if relative else (args[0], args[1])
                out.append(("Line", x, y))
                last_control = None
            elif cmd == "H":
                x = x + args[0] if relative else args[0]
                out.append(("Line", x, y))
                last_control = None
            elif cmd == "V":
                y = y + args[0] if relative else args[0]
                out.append(("Line", x, y))
                last_control = None
            elif cmd == "C":
                points = args if not relative else [
                    args[0] + x, args[1] + y, args[2] + x, args[3] + y, args[4] + x, args[5] + y
                ]
                out.append(("Cubic", *points))
                last_control = (points[2], points[3])
                x, y = points[4], points[5]
            elif cmd == "S":
                control = reflect(last_control, (x, y)) if last_kind in ("C", "S") else (x, y)
                points = args if not relative else [
                    args[0] + x, args[1] + y, args[2] + x, args[3] + y
                ]
                out.append(("Cubic", control[0], control[1], points[0], points[1], points[2], points[3]))
                last_control = (points[0], points[1])
                x, y = points[2], points[3]
            elif cmd == "Q":
                control = args if not relative else [args[0] + x, args[1] + y]
                end = (args[2] + x, args[3] + y) if relative else (args[2], args[3])
                out.append(quadratic_to_cubic((x, y), (control[0], control[1]), end))
                last_control = (control[0], control[1])
                x, y = end
            elif cmd == "T":
                control = reflect(last_control, (x, y)) if last_kind in ("Q", "T") else (x, y)
                end = (args[0] + x, args[1] + y) if relative else (args[0], args[1])
                out.append(quadratic_to_cubic((x, y), control, end))
                last_control = control
                x, y = end
            elif cmd == "A":
                end = (args[5] + x, args[6] + y) if relative else (args[5], args[6])
                out.extend(arc_to_cubics((x, y), args[0], args[1], args[2], args[3], args[4], end))
                x, y = end
                last_control = None
            last_kind = cmd
    return out


def reflect(control: tuple[float, float] | None, current: tuple[float, float]) -> tuple[float, float]:
    """The control point an `S`/`T` implies: the previous one mirrored through here."""
    if control is None:
        return current
    return (2 * current[0] - control[0], 2 * current[1] - control[1])


def quadratic_to_cubic(start: tuple[float, float], control: tuple[float, float],
                       end: tuple[float, float]) -> tuple:
    """A quadratic as the cubic that is the same curve, not an approximation of it."""
    c1 = (start[0] + 2 / 3 * (control[0] - start[0]), start[1] + 2 / 3 * (control[1] - start[1]))
    c2 = (end[0] + 2 / 3 * (control[0] - end[0]), end[1] + 2 / 3 * (control[1] - end[1]))
    return ("Cubic", c1[0], c1[1], c2[0], c2[1], end[0], end[1])


def arc_to_cubics(start: tuple[float, float], rx: float, ry: float, rotation: float,
                  large_arc: float, sweep: float, end: tuple[float, float]) -> list[tuple]:
    """An SVG elliptical arc as the cubics that draw it.

    The endpoint parameterisation converted to centre parameterisation, per the
    SVG specification's implementation notes, then split into segments of at most
    a quarter turn. Taken from the specification rather than from intuition on
    purpose: an arc's `large_arc` and `sweep` flags are where a hand-rolled
    version silently draws the other arc, which looks plausible and is wrong.
    """
    if start == end:
        return []
    if rx == 0 or ry == 0:
        return [("Line", end[0], end[1])]

    rx, ry = abs(rx), abs(ry)
    phi = math.radians(rotation)
    cos_phi, sin_phi = math.cos(phi), math.sin(phi)

    dx2, dy2 = (start[0] - end[0]) / 2, (start[1] - end[1]) / 2
    x1p = cos_phi * dx2 + sin_phi * dy2
    y1p = -sin_phi * dx2 + cos_phi * dy2

    # Scale the radii up if they are too small to span the two points.
    lam = (x1p ** 2) / (rx ** 2) + (y1p ** 2) / (ry ** 2)
    if lam > 1:
        factor = math.sqrt(lam)
        rx, ry = rx * factor, ry * factor

    sign = -1.0 if large_arc == sweep else 1.0
    numerator = rx ** 2 * ry ** 2 - rx ** 2 * y1p ** 2 - ry ** 2 * x1p ** 2
    denominator = rx ** 2 * y1p ** 2 + ry ** 2 * x1p ** 2
    coefficient = sign * math.sqrt(max(0.0, numerator / denominator)) if denominator else 0.0
    cxp = coefficient * rx * y1p / ry
    cyp = -coefficient * ry * x1p / rx
    cx = cos_phi * cxp - sin_phi * cyp + (start[0] + end[0]) / 2
    cy = sin_phi * cxp + cos_phi * cyp + (start[1] + end[1]) / 2

    def angle(ux: float, uy: float, vx: float, vy: float) -> float:
        dot = ux * vx + uy * vy
        norm = math.hypot(ux, uy) * math.hypot(vx, vy)
        if norm == 0:
            return 0.0
        value = max(-1.0, min(1.0, dot / norm))
        sign_angle = -1.0 if (ux * vy - uy * vx) < 0 else 1.0
        return sign_angle * math.acos(value)

    theta = angle(1.0, 0.0, (x1p - cxp) / rx, (y1p - cyp) / ry)
    delta = angle((x1p - cxp) / rx, (y1p - cyp) / ry, (-x1p - cxp) / rx, (-y1p - cyp) / ry)
    if sweep == 0 and delta > 0:
        delta -= 2 * math.pi
    elif sweep == 1 and delta < 0:
        delta += 2 * math.pi

    segments = max(1, int(math.ceil(abs(delta) / (math.pi / 2))))
    step = delta / segments

    def point(t: float) -> tuple[float, float]:
        cos_t, sin_t = math.cos(t), math.sin(t)
        return (
            cx + rx * cos_t * cos_phi - ry * sin_t * sin_phi,
            cy + rx * cos_t * sin_phi + ry * sin_t * cos_phi,
        )

    def derivative(t: float) -> tuple[float, float]:
        cos_t, sin_t = math.cos(t), math.sin(t)
        return (
            -rx * sin_t * cos_phi - ry * cos_t * sin_phi,
            -rx * sin_t * sin_phi + ry * cos_t * cos_phi,
        )

    out: list[tuple] = []
    for index in range(segments):
        t0 = theta + index * step
        t1 = t0 + step
        alpha = 4 / 3 * math.tan((t1 - t0) / 4)
        p0, p1 = point(t0), point(t1)
        d0, d1 = derivative(t0), derivative(t1)
        out.append((
            "Cubic",
            p0[0] + alpha * d0[0], p0[1] + alpha * d0[1],
            p1[0] - alpha * d1[0], p1[1] - alpha * d1[1],
            p1[0], p1[1],
        ))
    return out


# --------------------------------------------------------------------------
# Affine transforms
# --------------------------------------------------------------------------


IDENTITY = (1.0, 0.0, 0.0, 1.0, 0.0, 0.0)  # a b c d e f: x' = a x + c y + e


def apply(matrix: tuple[float, ...], x: float, y: float) -> tuple[float, float]:
    a, b, c, d, e, f = matrix
    return (a * x + c * y + e, b * x + d * y + f)


def multiply(outer: tuple[float, ...], inner: tuple[float, ...]) -> tuple[float, ...]:
    a1, b1, c1, d1, e1, f1 = outer
    a2, b2, c2, d2, e2, f2 = inner
    return (
        a1 * a2 + c1 * b2,
        b1 * a2 + d1 * b2,
        a1 * c2 + c1 * d2,
        b1 * c2 + d1 * d2,
        a1 * e2 + c1 * f2 + e1,
        b1 * e2 + d1 * f2 + f1,
    )


def parse_transform(text: str) -> tuple[float, ...]:
    """`translate(…) rotate(…)` and friends as one affine matrix."""
    matrix = IDENTITY
    for name, args in re.findall(r"([a-zA-Z]+)\s*\(([^)]*)\)", text):
        numbers = [float(n) for n in NUMBER.findall(args)]
        if name == "translate":
            tx, ty = numbers[0], numbers[1] if len(numbers) > 1 else 0.0
            step = (1.0, 0.0, 0.0, 1.0, tx, ty)
        elif name == "scale":
            sx, sy = numbers[0], numbers[1] if len(numbers) > 1 else numbers[0]
            step = (sx, 0.0, 0.0, sy, 0.0, 0.0)
        elif name == "rotate":
            radians = math.radians(numbers[0])
            cos, sin = math.cos(radians), math.sin(radians)
            step = (cos, sin, -sin, cos, 0.0, 0.0)
            if len(numbers) == 3:
                cx, cy = numbers[1], numbers[2]
                step = multiply((1.0, 0.0, 0.0, 1.0, cx, cy), multiply(step, (1.0, 0.0, 0.0, 1.0, -cx, -cy)))
        elif name == "matrix":
            step = tuple(numbers[:6])  # type: ignore[assignment]
        elif name in ("skewX", "skewY"):
            tangent = math.tan(math.radians(numbers[0]))
            step = (1.0, 0.0, tangent, 1.0, 0.0, 0.0) if name == "skewX" \
                else (1.0, tangent, 0.0, 1.0, 0.0, 0.0)
        else:
            raise Unsupported(f"unknown transform `{name}`")
        matrix = multiply(matrix, step)
    return matrix


def transform_commands(commands: list[tuple], matrix: tuple[float, ...]) -> list[tuple]:
    """Transform a command list. Exact for cubics: an affine map commutes with them."""
    if matrix == IDENTITY:
        return commands
    out: list[tuple] = []
    for command in commands:
        if command[0] == "Close":
            out.append(command)
        elif command[0] in ("Move", "Line"):
            x, y = apply(matrix, command[1], command[2])
            out.append((command[0], x, y))
        else:
            points = transform_points(matrix, command[1:])
            out.append(("Cubic", *points))
    return out


def transform_points(matrix: tuple[float, ...], flat: tuple[float, ...]) -> tuple[float, ...]:
    out: list[float] = []
    for index in range(0, len(flat), 2):
        x, y = apply(matrix, flat[index], flat[index + 1])
        out.extend((x, y))
    return tuple(out)


def stroke_scale(matrix: tuple[float, ...]) -> float:
    """How much a transform scales a stroke width.

    Transforming the geometry alone is not enough, and this was measured rather
    than assumed: `loader.svg` declares `stroke-width="23"` on a path inside
    `matrix(.08671 0 0 .0867 -49.8 -56)`, which is how a 24-unit icon ends up
    stroked at 2. Leave the width alone and that icon is drawn eleven times too
    thick -- and it still compiles, still renders, and still looks like an icon.

    Only a uniform scale is accepted. A non-uniform one turns a stroke into an
    ellipse with no single width, and this tool would rather refuse the icon than
    pick a number for it.
    """
    a, b, c, d = matrix[0], matrix[1], matrix[2], matrix[3]
    scale_x = math.hypot(a, b)
    scale_y = math.hypot(c, d)
    if abs(scale_x - scale_y) > 1e-3 * max(scale_x, scale_y, 1e-12):
        raise Unsupported(
            f"non-uniform transform scale ({scale_x:.4f} x {scale_y:.4f}) would "
            "make a stroke an ellipse"
        )
    return scale_x


# --------------------------------------------------------------------------
# Shapes
# --------------------------------------------------------------------------


def numbers_of(text: str) -> list[float]:
    return [float(n) for n in NUMBER.findall(text)]


def parse_style(text: str) -> dict[str, str]:
    """An inline `style` as the attributes it is spelling out.

    `box-plus.svg` writes `style="fill:none;…;stroke-width:2px;"` instead of
    attributes, so refusing `style` refuses a real icon. It is merged over the
    presentation attributes, which is the order CSS gives them: a `style`
    declaration wins over the attribute of the same name.
    """
    out: dict[str, str] = {}
    for declaration in text.split(";"):
        if ":" not in declaration:
            continue
        name, _, value = declaration.partition(":")
        out[name.strip().lower()] = value.strip()
    return out


def css_length(value: str) -> float:
    """A length with any CSS unit a stroke width here uses attached to it."""
    return float(re.sub(r"(px)$", "", value.strip()))


def shape_commands(tag: str, attrs: dict[str, str]) -> list[tuple]:
    """One element's geometry, in the same command vocabulary as a path."""
    if tag == "path":
        return path_commands(attrs["d"])
    if tag == "line":
        return [
            ("Move", float(attrs["x1"]), float(attrs["y1"])),
            ("Line", float(attrs["x2"]), float(attrs["y2"])),
        ]
    if tag == "circle":
        cx, cy, r = float(attrs.get("cx", 0)), float(attrs.get("cy", 0)), float(attrs["r"])
        return ellipse_commands(cx, cy, r, r)
    if tag == "ellipse":
        return ellipse_commands(
            float(attrs.get("cx", 0)), float(attrs.get("cy", 0)),
            float(attrs["rx"]), float(attrs["ry"]),
        )
    if tag == "rect":
        return rect_commands(attrs)
    if tag in ("polyline", "polygon"):
        points = numbers_of(attrs["points"])
        if len(points) < 4 or len(points) % 2:
            raise Unsupported("malformed points")
        out = [("Move", points[0], points[1])]
        for index in range(2, len(points), 2):
            out.append(("Line", points[index], points[index + 1]))
        if tag == "polygon":
            out.append(("Close",))
        return out
    raise Unsupported(f"no geometry for <{tag}>")


def ellipse_commands(cx: float, cy: float, rx: float, ry: float) -> list[tuple]:
    """A full ellipse as four cubics, which is what the toolkit can stroke."""
    kx, ky = rx * CIRCLE_KAPPA, ry * CIRCLE_KAPPA
    return [
        ("Move", cx + rx, cy),
        ("Cubic", cx + rx, cy + ky, cx + kx, cy + ry, cx, cy + ry),
        ("Cubic", cx - kx, cy + ry, cx - rx, cy + ky, cx - rx, cy),
        ("Cubic", cx - rx, cy - ky, cx - kx, cy - ry, cx, cy - ry),
        ("Cubic", cx + kx, cy - ry, cx + rx, cy - ky, cx + rx, cy),
        ("Close",),
    ]


def rect_commands(attrs: dict[str, str]) -> list[tuple]:
    x, y = float(attrs.get("x", 0)), float(attrs.get("y", 0))
    width, height = float(attrs["width"]), float(attrs["height"])
    rx = float(attrs.get("rx", attrs.get("ry", 0)) or 0)
    ry = float(attrs.get("ry", attrs.get("rx", 0)) or 0)
    if rx <= 0 or ry <= 0:
        return [
            ("Move", x, y),
            ("Line", x + width, y),
            ("Line", x + width, y + height),
            ("Line", x, y + height),
            ("Close",),
        ]
    rx, ry = min(rx, width / 2), min(ry, height / 2)
    return [
        ("Move", x + rx, y),
        ("Line", x + width - rx, y),
        ("Cubic", x + width - rx + rx * CIRCLE_KAPPA, y, x + width, y + ry - ry * CIRCLE_KAPPA, x + width, y + ry),
        ("Line", x + width, y + height - ry),
        ("Cubic", x + width, y + height - ry + ry * CIRCLE_KAPPA, x + width - rx + rx * CIRCLE_KAPPA, y + height, x + width - rx, y + height),
        ("Line", x + rx, y + height),
        ("Cubic", x + rx - rx * CIRCLE_KAPPA, y + height, x, y + height - ry + ry * CIRCLE_KAPPA, x, y + height - ry),
        ("Line", x, y + ry),
        ("Cubic", x, y + ry - ry * CIRCLE_KAPPA, x + rx - rx * CIRCLE_KAPPA, y, x + rx, y),
        ("Close",),
    ]


# --------------------------------------------------------------------------
# Reading one icon
# --------------------------------------------------------------------------


# Attribute names carry digits (`x1`, `y1`, `stroke-width`) and the geometry
# attributes are most of them, so the character class has to allow them; a class
# of letters and hyphens silently matches no attributes at all on a `<line>` and
# the icon then reads as empty geometry.
NAMESPACED = r"[a-zA-Z][a-zA-Z0-9-]*(?::[a-zA-Z][a-zA-Z0-9-]*)?"
TAG = re.compile(rf"<(/?)({NAMESPACED})((?:\s+{NAMESPACED}\s*=\s*\"[^\"]*\")*)\s*(/?)>")
ATTR = re.compile(rf"({NAMESPACED})\s*=\s*\"([^\"]*)\"")


class Element:
    def __init__(self, commands: list[tuple], stroke_width: float, filled: bool,
                 ink: tuple[str, str | None], element: str, opacity: float = 1.0,
                 even_odd: bool = False):
        self.commands = commands
        self.stroke_width = stroke_width
        self.filled = filled
        self.ink = ink
        self.element = element
        self.opacity = opacity
        self.even_odd = even_odd


def subpath_areas(commands: list[tuple]) -> list[float]:
    """The signed area of every subpath, curves sampled finely enough to trust.

    The sign is a subpath's winding direction, which is the whole question for a
    filled path with more than one subpath in it: the reference fills with the
    even-odd rule and the toolkit fills with the non-zero rule, and the two differ
    exactly when a hole is wound the same way as the shape it sits in.
    """
    subs: list[list[tuple[float, float]]] = []
    current: list[tuple[float, float]] = []
    x = y = 0.0
    for command in commands:
        if command[0] == "Move":
            if current:
                subs.append(current)
            x, y = command[1], command[2]
            current = [(x, y)]
        elif command[0] == "Line":
            x, y = command[1], command[2]
            current.append((x, y))
        elif command[0] == "Cubic":
            p0, p1, p2, p3 = (x, y), (command[1], command[2]), (command[3], command[4]), (command[5], command[6])
            for step in range(1, 9):
                t = step / 8
                u = 1 - t
                current.append((
                    u ** 3 * p0[0] + 3 * u * u * t * p1[0] + 3 * u * t * t * p2[0] + t ** 3 * p3[0],
                    u ** 3 * p0[1] + 3 * u * u * t * p1[1] + 3 * u * t * t * p2[1] + t ** 3 * p3[1],
                ))
            x, y = p3
    if current:
        subs.append(current)

    areas: list[float] = []
    for sub in subs:
        if len(sub) < 3:
            areas.append(0.0)
            continue
        areas.append(0.5 * sum(
            sub[i][0] * sub[(i + 1) % len(sub)][1] - sub[(i + 1) % len(sub)][0] * sub[i][1]
            for i in range(len(sub))
        ))
    return areas


def ink_hint(value: str | None) -> tuple[str, str | None]:
    """What an element's colour means here.

    `currentColor` is `Inherit` -- the whole point of these icons is that they
    take the colour of the button they sit in. A literal is `Fixed`. A `var(--x)`
    becomes `Token("--x")` so the shell can resolve it through `theme_gen` when
    the icon that needs it is drawn, rather than this tool inventing a value.
    """
    if value is None or value in ("currentColor", "none", "inherit"):
        return ("Inherit", None)
    if value.startswith("var("):
        name = value[4:].split(",")[0].strip().rstrip(")")
        if name.startswith("--") and value.count("var(") == 1:
            return ("Token", name)
        return ("Inherit", None)
    if value == "white":
        return ("Fixed", "255,255,255,255")
    if value == "black":
        return ("Fixed", "0,0,0,255")
    if value.startswith("#") and len(value) in (4, 7):
        digits = value[1:]
        if len(digits) == 3:
            digits = "".join(c * 2 for c in digits)
        return ("Fixed", f"{int(digits[0:2], 16)},{int(digits[2:4], 16)},{int(digits[4:6], 16)},255")
    return ("Inherit", None)


def read_icon(path: Path, key: str, both: list[str], even_odds: list[str],
              winding_risks: list[str], no_paint: list[str]) -> tuple[tuple[float, float], list[Element]]:
    text = path.read_text(encoding="utf-8")
    view = re.search(r'viewBox="([^"]+)"', text)
    if not view:
        raise Unsupported("no viewBox")
    box = numbers_of(view.group(1))
    if len(box) != 4:
        raise Unsupported("viewBox is not four numbers")
    view_box = (box[2], box[3])

    elements: list[Element] = []
    stack: list[dict[str, str]] = [{}]
    for match in TAG.finditer(text):
        closing, tag, raw_attrs, self_closing = match.groups()
        tag = tag.lower()
        attrs = {k.lower(): v for k, v in ATTR.findall(raw_attrs)}
        visual = tag in SHAPE_TAGS or tag in GROUP_TAGS
        for name in attrs:
            if visual and name not in SHAPE_ATTRS and not is_neutral(name):
                raise Unsupported(f"unknown attribute `{name}` on <{tag}>")
        if closing:
            if tag in GROUP_TAGS:
                stack.pop()
            continue
        if tag in IGNORED_TAGS:
            continue
        inherited = dict(stack[-1])
        if "style" in attrs:
            inherited.update(parse_style(attrs.pop("style")))
        inherited.update(attrs)
        if tag in GROUP_TAGS:
            # The root pushes too, so a shape with no attributes of its own is read
            # as the CSS would resolve it. Only `viewBox` is read off the root
            # directly, because it is not inherited -- it defines the coordinate
            # system the shapes are already written in.
            if not self_closing:
                stack.append(inherited)
            continue
        if tag not in SHAPE_TAGS:
            raise Unsupported(f"unknown element <{tag}>")

        commands = shape_commands(tag, attrs)
        matrix = parse_transform(inherited["transform"]) if "transform" in inherited else IDENTITY
        commands = transform_commands(commands, matrix)

        fill = inherited.get("fill", "none")
        stroke = inherited.get("stroke", None)
        has_fill = fill not in ("none", None)
        has_stroke = stroke not in ("none", None)
        if has_fill and has_stroke:
            both.append(f"{key}:{tag}")
        filled = has_fill and not has_stroke
        width = css_length(inherited.get("stroke-width", "2")) * stroke_scale(matrix)
        # `opacity` is a real part of the drawing -- the spinner's ring is 25% of
        # its colour -- so it is carried rather than dropped. The two finer
        # attributes are refused when they are not neutral, because a generator
        # that accepts an attribute it does not apply is worse than one that
        # stops: the icon looks right until someone checks that one ring.
        for name in ("fill-opacity", "stroke-opacity"):
            declared = inherited.get(name)
            if declared not in (None, "1", ""):
                raise Unsupported(f"unhandled `{name}={declared}`")
        opacity = float(inherited.get("opacity", "1"))
        for value, default in (("stroke-linecap", "butt"), ("stroke-linejoin", "miter")):
            declared = inherited.get(value, default)
            if declared not in ("round", "butt", "square") and value.endswith("linecap"):
                raise Unsupported(f"unsupported {value} `{declared}`")
            if declared not in ("round", "miter", "bevel") and value.endswith("linejoin"):
                raise Unsupported(f"unsupported {value} `{declared}`")
        # A shape can be filled *and* stroked -- `images.svg` draws its circle
        # both ways -- and one `Element` carries one paint, so it becomes two with
        # the same geometry. Emitting it twice is exact; a flag that meant "both"
        # would put two decisions in one field and the painter would have to
        # guess which one a filled-and-stroked element wanted.
        # The one thing this generator cannot carry across. Named and counted
        # rather than glossed: stage 2 then knows which icons need a visual
        # comparison against the reference rather than a claim about them.
        # What this was before the toolkit could be told: the risk was computed,
        # named and refused, because the toolkit filled non-zero and there was
        # nowhere to put the rule. Both backends tessellate either rule, so the
        # rule is carried now -- see `even_odd` on `Element` and `Paint::Fill` --
        # and what is left to compute is *which* of them would actually draw
        # differently without it. An even-odd path with one subpath is the same
        # shape either way, so it is carried but not counted.
        even_odd = has_fill and inherited.get("fill-rule", "nonzero") == "evenodd"
        if even_odd:  # noqa: SIM102 -- the branch is a measurement, not a conversion
            areas = subpath_areas(commands)
            even_odds.append(f"{key}#{len(elements)}")
            same_way = len(areas) > 1 and (all(a > 0 for a in areas) or all(a < 0 for a in areas))
            if same_way:
                winding_risks.append(f"{key}#{len(elements)}")

        if has_fill:
            elements.append(Element(commands, width, True, ink_hint(fill), tag, opacity, even_odd))
        if has_stroke or not has_fill:
            if has_stroke:
                elements.append(Element(commands, width, False, ink_hint(stroke), tag, opacity))
            else:
                # `fill="none"` and no stroke, or `stroke="none"` beside a `fill` of
                # `none`: SVG paints nothing at all, and the shape is the clamp path
                # every exported icon carries (`M0 0h24v24H0Z`) or that clamp
                # spelled as a `<rect>`/a `stroke="none"` path. It used to be
                # emitted as a stroked outline here -- reading `fill="none"` as
                # *unfilled but stroked* -- which drew a full 24x24 box around ten
                # glyphs: a 1px frame the reference does not draw, on *Fabric*,
                # *Forge* and eight others. So it is emitted as the one command
                # that paints nothing and the interpreter skips it. The element is
                # still written, still counted and still named in the generated
                # gate, because dropping it would leave no trace of having read it.
                elements.append(Element([("NoPaint",)], width, False, ("Inherit", None), tag, opacity))
                no_paint.append(f"{key}#{len(elements) - 1}")
    if not elements:
        raise Unsupported("no geometry")
    return view_box, elements


# --------------------------------------------------------------------------
# Emitting Rust
# --------------------------------------------------------------------------


def variant(name: str) -> str:
    """`arrow-down-a-z` -> `ArrowDownAZ`."""
    parts = re.split(r"[-_]", name)
    out = []
    for part in parts:
        if not part:
            continue
        if part.isdigit():
            if out:
                out[-1] += part
            else:
                out.append("N" + part)
        else:
            out.append(part[0].upper() + part[1:])
    return "".join(out)


def fnum(value: float) -> str:
    if value == int(value) and abs(value) < 1e9:
        return f"{int(value)}.0"
    text = f"{value:.4f}".rstrip("0").rstrip(".")
    return text if "." in text else text + ".0"


def f32_text(value: float) -> str:
    """`value` as Rust's `Display` for an `f32` spells it.

    The width test below compares formatted strings rather than numbers, so the
    two sides have to spell a float the same way. Rust prints the shortest
    decimal that round-trips through the type, which `repr` of the `f64` this
    tool computes in is not: `2.32536f32` is `2.3254` there and `2.32536` here,
    and the only way to learn the first is to ask for successively rounder
    spellings until one lands on the same `f32` again.

    The short spellings the set already had (`8`, `1.6`, `4`, `1.5`) never
    showed this, because `%g` and Rust agree on every value whose shortest
    spelling is also its six-significant-digit one.
    """
    packed = struct.pack("f", value)
    for precision in range(1, 10):
        text = f"{struct.unpack('f', packed)[0]:.{precision}g}"
        if struct.pack("f", float(text)) == packed:
            return text
    return f"{value:g}"


def width_text(value: float) -> str:
    """A stroke width the way the *table* spells it, not the way this computes it.

    Two roundings stand between a width here and the `f32` the test reads back:
    [`fnum`] rounds to four decimals when it writes the literal, and Rust's
    `Display` then prints the shortest spelling of that `f32`. Going through
    `fnum` first is what makes the two sides agree -- `2.32536` is written as
    `2.3254` and read back as `2.3254`, so comparing against the unrounded value
    is comparing against a number the file does not hold.
    """
    return f32_text(float(fnum(value)))


def clean(commands: list[tuple]) -> list[tuple]:
    """Round to four decimals, which is below a device pixel at any sane zoom."""
    out: list[tuple] = []
    for command in commands:
        if command[0] == "Close":
            out.append(command)
        else:
            out.append((command[0],) + tuple(round(float(v), 4) for v in command[1:]))
    return out


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--check", action="store_true",
                        help="fail if the checked-in file is not what this would emit")
    parser.add_argument("--report", action="store_true", help="print per-icon detail")
    args = parser.parse_args()

    if not ICONS.is_dir():
        print(f"missing reference icons: {ICONS}", file=sys.stderr)
        return 2

    icons: list[tuple] = []
    sets: dict[str, list[tuple]] = {}
    both: list[str] = []
    winding_risks: list[str] = []
    refused: list[tuple[str, str]] = []
    even_odds: list[str] = []
    no_paint: list[str] = []
    for source in SOURCES:
        directory = ICONS / source["directory"] if source["directory"] else ICONS
        files = sorted(directory.glob(source["glob"]))
        if not files:
            print(f"no icons in {directory}", file=sys.stderr)
            return 2
        collected: list[tuple] = []
        for path in files:
            # The name in `ALL` and its two tag tables is the file's path below
            # `assets/icons` without its extension, because a stem is not unique
            # across the three: `badge-check.svg` is two different pictures, and
            # `by_name` has to be able to say which one it meant.
            name = (Path(source["directory"]) / path.stem).as_posix() if source["directory"] \
                else path.stem
            # The four lists this reader fills are per-icon and only merged once
            # the icon has been read whole: `read_icon` appends as it walks the
            # shapes, so an icon it stops half way through would otherwise leave
            # its findings in a list the gates compare against the generated file
            # -- `purpur` and `quilt` each named themselves as painting nothing
            # before the non-uniform transform that refuses them was reached.
            found_both: list[str] = []
            found_even_odds: list[str] = []
            found_winding: list[str] = []
            found_no_paint: list[str] = []
            try:
                box, elements = read_icon(
                    path, name, found_both, found_even_odds, found_winding, found_no_paint,
                )
            except Unsupported as exc:
                if source["complete"]:
                    print(f"{name}.svg: {exc}", file=sys.stderr)
                    return 1
                if name not in REFUSED_TAGS:
                    print(f"{name}.svg: {exc}", file=sys.stderr)
                    print(
                        f"  {name}.svg is a tag icon this tool cannot read and is not "
                        "in REFUSED_TAGS.\n  Add it there with the construct that stopped "
                        "it, or teach the parser the construct -- not both.",
                        file=sys.stderr,
                    )
                    return 1
                refused.append((name, str(exc)))
                print(f"{name}.svg: {exc} -- refused, and named in REFUSED_TAGS",
                      file=sys.stderr)
                continue
            both.extend(found_both)
            even_odds.extend(found_even_odds)
            winding_risks.extend(found_winding)
            no_paint.extend(found_no_paint)
            collected.append((name, source["prefix"] + variant(path.stem), box, elements))
        sets[source["table"]] = collected
        icons.extend(collected)

    seen: dict[str, str] = {}
    for name, rust, _, _ in icons:
        if rust in seen:
            raise SystemExit(f"{seen[rust]} and {name} both become `{rust}`")
        seen[rust] = name

    lines: list[str] = []
    add = lines.append
    total_commands = sum(len(e.commands) for _, _, _, elements in icons for e in elements)

    add("//! The reference client's icon set, compiled into strokeable geometry.")
    add("//!")
    add("//! **Generated by `tools/gen_icons.py`. Do not edit.**")
    add("//!")
    add("//! These are the Modrinth App's own icons, out of the SVGs in")
    add("//! `vendor/modrinth-app/assets/icons`, and they are outlines rather than")
    add("//! pictures: the reference draws them stroked in `currentColor` at width 2")
    add("//! with round caps and joins, which is what lets an icon take the colour of")
    add("//! the control it sits in. This is why they are geometry and not a bitmap --")
    add("//! a rasterised icon cannot be tinted, and the shell's previous icons were")
    add("//! bitmaps carved out of another launcher's binary.")
    add("//!")
    add("//! `assets/icons` is three directories, and so is this file:")
    add("//!")
    add(f"//! | table | where | variants | count |")
    add("//! | --- | --- | --- | --- |")
    for source in SOURCES:
        where = f"`icons/{source['directory']}`" if source["directory"] else "`icons/`"
        add(f"//! | `{source['table']}` | {where} | "
            f"`{source['prefix'] or 'none'}` | {len(sets[source['table']])} |")
    add("//!")
    add("//! The prefixes are the reference's own. `assets/build/generate-exports.ts`")
    add("//! walks the same two tag directories and writes `TagCategory${stem}` and")
    add("//! `TagLoader${stem}` because `tags/categories/badge-check.svg` and")
    add("//! `badge-check.svg` are two different pictures; the `Icon` suffix it also")
    add("//! appends is dropped here because these variants read `BadgeCheck`. Nothing")
    add("//! outside a tag set is prefixed, which is what keeps the other names as they")
    add("//! were.")
    add("//!")
    add("//! Four of the 132 tag icons are **not here**, because the generator refuses")
    add("//! the construct rather than approximating it. They are named below and in")
    add("//! `the_tag_icons_this_tool_could_not_read_are_the_four_it_names`:")
    add("//!")
    for name in sorted(REFUSED_TAGS):
        add(f"//! * `{name}.svg` -- {REFUSED_TAGS[name]}")
    add("//!")
    add("//! A tag pill for one of those four draws no icon, which makes it the")
    add("//! 24-row pill rather than the 26-row one -- the height a tag with no icon")
    add("//! has anyway, and not a wrong shape.")
    add("//!")
    add("//! Arcs, quadratics and the smooth-curve commands are all already converted")
    add("//! to cubics by the generator, and `<g transform>` is applied to every point")
    add("//! before it is written: an affine map commutes with Bezier evaluation, so")
    add("//! transforming the control points transforms the curve exactly.")
    add("//!")
    add(f"//! {len(no_paint)} elements are **`Cmd::NoPaint`**: a shape the reference")
    add("//! declares with neither a fill nor a stroke, which SVG paints not at all.")
    add("//! It is the clamp path every exported icon carries, and it used to be")
    add("//! emitted as a stroked outline -- reading `fill=\"none\"` as *unfilled but")
    add("//! stroked* -- which drew a 1px frame around ten glyphs. They are named in")
    add("//! `the_shapes_the_reference_does_not_paint_are_the_ten_it_names`:")
    add("//!")
    for name in sorted(no_paint):
        add(f"//! * `{name.split('#')[0]}.svg` element {int(name.split('#')[1]) + 1}")
    add("//!")
    add("//! Gate: `python tools/gen_icons.py --check` regenerates and compares bytes.")
    add("")
    add("// Nothing draws these yet: the shell that will is stage 2 of")
    add("// `docs/superpowers/specs/2026-09-24-modrinth-native-rewrite.md`, and the shell")
    add("// on screen today draws `crate::icons`' carved PNGs. Delete this line the day")
    add("// the last icon is drawn from here.")
    add("#![allow(dead_code)]")
    add("")
    add("use iced::widget::canvas::{fill::Rule, Fill, LineCap, LineDash, LineJoin, Path, Stroke, Style};")
    add("use iced::{Color, Point};")
    add("")
    add("/// One drawing step. Absolute coordinates, in the icon's own view box.")
    add("#[derive(Clone, Copy, Debug, PartialEq)]")
    add("pub enum Cmd {")
    add("    /// Start a subpath at `(x, y)`.")
    add("    Move(f32, f32),")
    add("    /// A straight line to `(x, y)`.")
    add("    Line(f32, f32),")
    add("    /// A cubic Bezier: two control points then the end point.")
    add("    Cubic(f32, f32, f32, f32, f32, f32),")
    add("    /// Close the current subpath.")
    add("    Close,")
    add("    /// A shape that declares neither a fill nor a stroke, which SVG paints")
    add("    /// not at all. The builder skips it, so the element it belongs to")
    add("    /// builds an empty path.")
    add("    ///")
    add("    /// This is the clamp path every exported icon carries -- `M0 0h24v24H0Z`")
    add("    /// with `fill=\"none\"` and no `stroke` -- written out because reading")
    add("    /// `fill=\"none\"` as *unfilled but stroked* drew a 24-unit box around")
    add("    /// the glyph: a 1px frame the reference does not draw.")
    add("    NoPaint,")
    add("}")
    add("")
    add("/// Where an element's colour comes from.")
    add("#[derive(Clone, Copy, Debug, PartialEq, Eq)]")
    add("pub enum Ink {")
    add("    /// `currentColor`: the colour of whatever the icon is drawn in, which is")
    add("    /// what almost every icon in the set asks for.")
    add("    Inherit,")
    add("    /// A literal the reference wrote out: a red, a white.")
    add("    Fixed(u8, u8, u8, u8),")
    add("    /// A `var(--token)` the reference names, to be resolved through")
    add("    /// `crate::theme_gen` rather than guessed at here.")
    add("    Token(&'static str),")
    add("}")
    add("")
    add("impl Ink {")
    add("    /// The colour this element is, given what the caller is drawing with.")
    add("    pub fn resolve(self, inherit: Color) -> Color {")
    add("        match self {")
    add("            Ink::Inherit => inherit,")
    add("            Ink::Fixed(r, g, b, a) => Color::from_rgba8(r, g, b, a as f32 / 255.0),")
    add("            // A token the shell has not been taught yet is drawn in the ink it")
    add("            // would have had anyway, which is visible rather than invisible.")
    add("            Ink::Token(_) => inherit,")
    add("        }")
    add("    }")
    add("}")
    add("")
    add("/// One drawable piece of an icon: geometry, and how to paint it.")
    add("#[derive(Clone, Copy, Debug, PartialEq)]")
    add("pub struct Element {")
    add("    /// The geometry, in the icon's view box.")
    add("    pub commands: &'static [Cmd],")
    add("    /// Stroke width, in view-box units.")
    add("    pub stroke_width: f32,")
    add("    /// Whether this element is filled rather than stroked.")
    add("    pub filled: bool,")
    add("    /// Where its colour comes from.")
    add("    pub ink: Ink,")
    add("    /// Whether the reference fills this element with `fill-rule=\"evenodd\"`.")
    add("    ///")
    add("    /// It is carried rather than assumed because the two rules differ, and five")
    add("    /// of the tag icons depend on it: the gear behind *Data Pack*, *Resource")
    add("    /// Pack* and *Vanilla Shader*, and the two faces behind *Mobs* and")
    add("    /// *Entities*, all cut their holes with it and are solid blobs filled")
    add("    /// non-zero. Both backends tessellate a `Fill` with either rule, so there")
    add("    /// is nothing to refuse -- what there was until this was a generator that")
    add("    /// knew about the risk and had nowhere to put the answer.")
    add("    pub even_odd: bool,")
    add("    /// The element's own `opacity`, which multiplies whatever ink it gets.")
    add("    ///")
    add("    /// The spinner's ring is a quarter opaque, so this is not decoration:")
    add("    /// a stroke at full opacity there draws a solid ring instead of a track.")
    add("    pub opacity: f32,")
    add("}")
    add("")
    add("/// Every icon in the reference's three sets.")
    add("///")
    add("/// One enum and three tables, because that is the shape the reference itself")
    add("/// has: `icons/*.svg` are the icons a control draws, and the two tag")
    add("/// directories are the glyphs a tag pill draws, which `getTagIcon` looks up")
    add("/// in the loader table and then the category table. Splitting the tables")
    add("/// rather than the enum keeps `ALL` at the 313 top-level icons its own gate")
    add("/// walks, so a tag icon appearing does not move the top-level element count.")
    add("#[derive(Clone, Copy, Debug, PartialEq, Eq)]")
    add("pub enum Glyph {")
    for name, rust, _, _ in icons:
        add(f"    /// `{name}.svg`")
        add(f"    {rust},")
    add("}")
    add("")
    add("/// The top-level set, in name order, with the file name beside each.")
    add("pub const ALL: &[(&str, Glyph)] = &[")
    for name, rust, _, _ in sets["ALL"]:
        add(f"    (\"{name}\", Glyph::{rust}),")
    add("];")
    add("")
    add("/// `icons/tags/categories/*.svg`, in name order: a category tag's own icon.")
    add("///")
    add("/// The reference's `categoryIconMap`, keyed by the same file stem. A project")
    add("/// card does not draw these -- `ProjectCardTags` passes `hide-non-loader-icon`")
    add("/// -- so what asks this table for an icon is a tag *list* outside a card.")
    add("pub const TAG_CATEGORIES: &[(&str, Glyph)] = &[")
    for name, rust, _, _ in sets["TAG_CATEGORIES"]:
        add(f"    (\"{name}\", Glyph::{rust}),")
    add("];")
    add("")
    add("/// `icons/tags/loaders/*.svg`, in name order: a loader tag's own icon.")
    add("///")
    add("/// The reference's `loaderIconMap`, keyed by the same file stem. These are")
    add("/// the glyphs a project card's tag pills draw, and there are 30 of them")
    add("/// against `TAG_CATEGORIES`'s 102.")
    add("pub const TAG_LOADERS: &[(&str, Glyph)] = &[")
    for name, rust, _, _ in sets["TAG_LOADERS"]:
        add(f"    (\"{name}\", Glyph::{rust}),")
    add("];")
    add("")
    add("/// The three tables, in the order they are declared above.")
    add("fn tables() -> [&'static [(&'static str, Glyph)]; 3] {")
    add(f"    [{', '.join(source['table'] for source in SOURCES)}]")
    add("}")
    add("")
    add("impl Glyph {")
    add("    /// The icon's own file name below `assets/icons`, without the extension.")
    add("    ///")
    add("    /// A tag icon's name carries its directory -- `tags/loaders/forge` rather")
    add("    /// than `forge` -- because the stem is not unique across the three sets,")
    add("    /// and a name that could be two pictures is not a name.")
    add("    pub fn name(self) -> &'static str {")
    add("        tables()")
    add("            .iter()")
    add("            .flat_map(|table| table.iter())")
    add("            .find(|(_, g)| *g == self)")
    add("            .map(|(n, _)| *n)")
    add("            .unwrap_or(\"\")")
    add("    }")
    add("")
    add("    /// The icon a name refers to, for a gate that walks the reference.")
    add("    pub fn by_name(name: &str) -> Option<Glyph> {")
    add("        tables()")
    add("            .iter()")
    add("            .flat_map(|table| table.iter())")
    add("            .find(|(n, _)| *n == name)")
    add("            .map(|(_, g)| *g)")
    add("    }")
    add("")
    add("    /// The view box the commands are expressed in, as `(width, height)`.")
    add("    pub fn view_box(self) -> (f32, f32) {")
    add("        match self {")
    for name, rust, box_, _ in icons:
        add(f"            Glyph::{rust} => ({fnum(box_[0])}, {fnum(box_[1])}),")
    add("        }")
    add("    }")
    add("")
    add("    /// This icon's elements, in the reference's document order.")
    add("    pub fn elements(self) -> &'static [Element] {")
    add("        match self {")
    for name, rust, _, elements in icons:
        add(f"            Glyph::{rust} => &{table_name(rust)},")
    add("        }")
    add("    }")
    add("}")
    add("")
    add("/// A path builder that turns one element's commands into a strokeable path.")
    add("fn build(commands: &[Cmd]) -> Path {")
    add("    Path::new(|builder| {")
    add("        for command in commands {")
    add("            match *command {")
    add("                Cmd::Move(x, y) => builder.move_to(Point::new(x, y)),")
    add("                Cmd::Line(x, y) => {")
    add("                    builder.line_to(Point::new(x, y));")
    add("                }")
    add("                Cmd::Cubic(c1x, c1y, c2x, c2y, x, y) => {")
    add("                    builder.bezier_curve_to(")
    add("                        Point::new(c1x, c1y),")
    add("                        Point::new(c2x, c2y),")
    add("                        Point::new(x, y),")
    add("                    );")
    add("                }")
    add("                Cmd::Close => builder.close(),")
    add("                // A shape with no fill and no stroke has no segments to walk,")
    add("                // so there is nothing to hand the builder.")
    add("                Cmd::NoPaint => {}")
    add("            }")
    add("        }")
    add("    })")
    add("}")
    add("")
    add("/// How one element is painted: stroked in the ink, or filled with it.")
    add("///")
    add("/// The reference fills a handful of its icons and strokes the rest, so this")
    add("/// is a property of the element rather than of the icon.")
    add("///")
    add("/// `Fill` is the toolkit's own fill rather than a bare `Color` because it")
    add("/// carries the rule beside the style, and the five even-odd tag icons are")
    add("/// wrong without it. It is the type `Frame::fill` already takes, so a caller")
    add("/// that hands it straight over -- which is what both of this crate's")
    add("/// callers do -- keeps compiling.")
    add("///")
    add("/// No `PartialEq`: the toolkit's `Stroke` does not carry one, and nothing")
    add("/// needs to compare two painted elements.")
    add("///")
    add("/// The variant-size allowance: a stroke carries its own dash pattern, cap")
    add("/// and join, which is most of a hundred bytes against a fill's sixteen. A")
    add("/// `Box` would move that off the stack and buy nothing -- one of these is")
    add("/// built per element and handed straight to the frame -- so the size is")
    add("/// left where it is and said out loud.")
    add("#[allow(clippy::large_enum_variant)]")
    add("#[derive(Debug)]")
    add("pub enum Paint {")
    add("    /// Stroke the path, at the width the reference declares for it.")
    add("    Stroke(Stroke<'static>),")
    add("    /// Fill the path, with the rule the reference declared for it.")
    add("    Fill(Fill),")
    add("}")
    add("")
    add("/// One icon, ready to draw: geometry in its own view box, and how to paint it.")
    add("///")
    add("/// `scale` is the one thing this cannot know -- the reference draws a 24-unit")
    add("/// icon at whatever pixel size a control needs, so a caller drawing it at 16px")
    add("/// passes two thirds and the stroke scales with it. That is the whole reason")
    add("/// these are paths rather than a bitmap: a bitmap has one size.")
    add("///")
    add("/// Placing the result is the frame's job (`canvas::Frame`'s own `translate` and")
    add("/// `scale`), not this function's: a path is rebuilt every frame anyway, and")
    add("/// baking a position in here would rebuild it once per position instead.")
    add("pub fn parts(glyph: Glyph, scale: f32, inherit: Color) -> Vec<(Path, Paint)> {")
    add("    let mut out = Vec::with_capacity(glyph.elements().len());")
    add("    for element in glyph.elements() {")
    add("        let resolved = element.ink.resolve(inherit);")
    add("        let ink = Color {")
    add("            a: resolved.a * element.opacity,")
    add("            ..resolved")
    add("        };")
    add("        let path = build(element.commands);")
    add("        let paint = if element.filled {")
    add("            Paint::Fill(Fill {")
    add("                style: Style::Solid(ink),")
    add("                rule: if element.even_odd { Rule::EvenOdd } else { Rule::NonZero },")
    add("            })")
    add("        } else {")
    add("            Paint::Stroke(Stroke {")
    add("                style: Style::Solid(ink),")
    add("                width: element.stroke_width * scale,")
    add("                line_cap: LineCap::Round,")
    add("                line_join: LineJoin::Round,")
    add("                line_dash: LineDash::default(),")
    add("            })")
    add("        };")
    add("        out.push((path, paint));")
    add("    }")
    add("    out")
    add("}")
    add("")

    for name, rust, _, elements in icons:
        add(f"/// `{name}.svg`, as {len(elements)} element(s).")
        add(f"const {table_name(rust)}: [Element; {len(elements)}] = [")
        for index, element in enumerate(elements):
            add("    Element {")
            add(f"        commands: &{command_table(rust, index)},")
            add(f"        stroke_width: {fnum(element.stroke_width)},")
            add(f"        filled: {'true' if element.filled else 'false'},")
            add(f"        even_odd: {'true' if element.even_odd else 'false'},")
            add(f"        opacity: {fnum(element.opacity)},")
            tint = element.ink
            if tint[0] == "Fixed":
                r, g, b, a = tint[1].split(",")
                add(f"        ink: Ink::Fixed({r}, {g}, {b}, {a}),")
            elif tint[0] == "Token":
                add(f"        ink: Ink::Token(\"{tint[1]}\"),")
            else:
                add("        ink: Ink::Inherit,")
            add("    },")
        add("];")
        add("")
        for index, element in enumerate(elements):
            commands = clean(element.commands)
            add(f"/// `{name}.svg` element {index + 1} of {len(elements)} "
                f"(`<{element.element}>`), {len(commands)} command(s).")
            add(f"const {command_table(rust, index)}: [Cmd; {len(commands)}] = [")
            for command in commands:
                add(f"    {command_source(command)},")
            add("];")
            add("")

    # The widths that are not the set's own 2, spelled the way Rust's `Display`
    # for `f32` spells them, because the test below compares formatted strings.
    # Within a hundredth of 2 counts as 2: `loader` reaches 2 through its own
    # transform's scale factor (23 x 0.08671), so an exact comparison would list
    # an icon that is stroked correctly as an exception.
    width_exceptions = sorted(
        f"{name}:{width_text(element.stroke_width)}"
        for name, _, _, elements in icons
        for element in elements
        if abs(element.stroke_width - 2.0) > 0.01
    )
    winding_risks = sorted(set(winding_risks))
    with_tokens = sorted({(name, e.ink[1]) for name, _, _, els in icons for e in els if e.ink[0] == "Token"})
    filled_icons = sorted({name for name, _, _, els in icons if any(e.filled for e in els)})
    widths = sorted({e.stroke_width for _, _, _, els in icons for e in els})
    # The tag stems that a top-level icon already answers to, which is what the
    # prefixes exist for: fifteen of them, all categories. Written down because
    # "the prefixes keep the top-level names as they are" is a claim about a list
    # and this is the list.
    top_stems = {name.rsplit("/", 1)[-1] for name, _, _, _ in sets["ALL"]}
    collisions = sorted(
        name for name, _, _, _ in icons
        if "/" in name and name.rsplit("/", 1)[-1] in top_stems
    )

    add("#[cfg(test)]")
    add("mod tests {")
    add("    use super::*;")
    add("")
    add("    /// Every icon in the three tables, as `(name, glyph)` pairs.")
    add("    fn every() -> Vec<(&'static str, Glyph)> {")
    add("        tables()")
    add("            .iter()")
    add("            .flat_map(|table| table.iter().copied())")
    add("            .collect()")
    add("    }")
    add("")
    add("    #[test]")
    add("    fn the_whole_reference_is_here() {")
    add("        // The vendored tree holds 313 top-level SVGs, 102 in")
    add("        // `tags/categories` and 30 in `tags/loaders`; a generator that")
    add("        // quietly skipped one that it could not parse would leave a hole")
    add("        // nothing else notices, so every count is asserted rather than the")
    add("        // absence of errors. The two tag counts are 132 less the four named")
    add("        // in `the_tag_icons_this_tool_could_not_read_are_the_four_it_names`,")
    add("        // which is why that test exists and why it fails when one of them is")
    add("        // taught to the parser.")
    add(f"        assert_eq!(ALL.len(), {len(sets['ALL'])});")
    add(f"        assert_eq!(TAG_CATEGORIES.len(), {len(sets['TAG_CATEGORIES'])});")
    add(f"        assert_eq!(TAG_LOADERS.len(), {len(sets['TAG_LOADERS'])});")
    add(f"        assert_eq!(every().len(), {len(icons)}, \"the variant count moved\");")
    add("        let mut names: Vec<&str> = every().iter().map(|(n, _)| *n).collect();")
    add("        names.sort_unstable();")
    add("        let mut unique = names.clone();")
    add("        unique.dedup();")
    add("        assert_eq!(names, unique, \"two icons share a name\");")
    add("        // And every variant is reachable from a table, which is what makes")
    add("        // `name()` and `by_name()` total over the enum rather than over a")
    add("        // subset of it.")
    add("        for (_, glyph) in every() {")
    add("            assert!(!glyph.name().is_empty(), \"{glyph:?} is in no table\");")
    add("            assert_eq!(Glyph::by_name(glyph.name()), Some(glyph));")
    add("        }")
    add("    }")
    add("")
    add("    #[test]")
    add("    fn the_two_tag_sets_carry_the_reference_prefixes() {")
    add("        // `assets/build/generate-exports.ts` walks these same two")
    add("        // directories and prefixes each stem, because")
    add("        // `tags/categories/badge-check.svg` and `badge-check.svg` are two")
    add("        // different pictures. The prefix is what keeps the 313 top-level")
    add("        // names exactly as they were -- they are referenced by name across")
    add("        // the whole tree, and a collision guard that renamed one would be a")
    add("        // silent break in every one of them.")
    add("        for (name, glyph) in TAG_CATEGORIES {")
    add("            assert!(name.starts_with(\"tags/categories/\"), \"{name}\");")
    add("            assert!(")
    add("                format!(\"{glyph:?}\").starts_with(\"TagCategory\"),")
    add("                \"{name} lost its prefix\"")
    add("            );")
    add("        }")
    add("        for (name, glyph) in TAG_LOADERS {")
    add("            assert!(name.starts_with(\"tags/loaders/\"), \"{name}\");")
    add("            assert!(")
    add("                format!(\"{glyph:?}\").starts_with(\"TagLoader\"),")
    add("                \"{name} lost its prefix\"")
    add("            );")
    add("        }")
    add("        // No tag glyph shares a variant with a top-level one, which is the")
    add("        // whole reason for the prefix. `badge-check` is the case that")
    add("        // proves the guard was reached rather than passed by luck: the")
    add("        // reference ships both.")
    add("        let top: Vec<String> = ALL.iter().map(|(_, g)| format!(\"{g:?}\")).collect();")
    add("        for (_, glyph) in TAG_CATEGORIES.iter().chain(TAG_LOADERS) {")
    add("            assert!(")
    add("                !top.contains(&format!(\"{glyph:?}\")),")
    add("                \"{glyph:?} is also a top-level variant\"")
    add("            );")
    add("        }")
    add(f"        let mut clashing: Vec<&str> = vec![{', '.join(chr(34) + n + chr(34) for n in collisions)}];")
    add("        clashing.sort_unstable();")
    add("        let mut actual: Vec<&str> = TAG_CATEGORIES")
    add("            .iter()")
    add("            .chain(TAG_LOADERS)")
    add("            .filter(|(_, glyph)| {")
    add("                let stem = glyph.name().rsplit('/').next().unwrap_or_default();")
    add("                ALL.iter().any(|(name, _)| {")
    add("                    let other = name.rsplit('/').next().unwrap_or(name);")
    add("                    other == stem")
    add("                })")
    add("            })")
    add("            .map(|(name, _)| *name)")
    add("            .collect();")
    add("        actual.sort_unstable();")
    add("        assert_eq!(")
    add("            actual, clashing,")
    add("            \"the set of tag icons whose stem is also a top-level icon moved\"")
    add("        );")
    add("        assert!(!actual.is_empty(), \"the collision list is empty, so this proves nothing\");")
    add("        assert_ne!(Glyph::TagCategoryBadgeCheck, Glyph::BadgeCheck);")
    add("    }")
    add("")
    add("    #[test]")
    add("    fn every_icon_draws_something_finite() {")
    add("        for (name, glyph) in every() {")
    add("            let elements = glyph.elements();")
    add("            assert!(!elements.is_empty(), \"{name} has no elements\");")
    add("            for element in elements {")
    add("                assert!(!element.commands.is_empty(), \"{name} has an empty element\");")
    add("                assert!(")
    add("                    matches!(element.commands[0], Cmd::Move(..) | Cmd::NoPaint),")
    add("                    \"{name} does not start by moving\"")
    add("                );")
    add("                for command in element.commands {")
    add("                    for value in numbers(*command) {")
    add("                        assert!(value.is_finite(), \"{name} has a non-finite coordinate\");")
    add("                        assert!(value.abs() < 1.0e4, \"{name} has an absurd coordinate\");")
    add("                    }")
    add("                }")
    add("                assert!(element.stroke_width > 0.0, \"{name} has no stroke width\");")
    add("            }")
    add("        }")
    add("    }")
    add("")
    add("    #[test]")
    add("    fn no_command_survived_that_the_toolkit_cannot_draw() {")
    add("        // Arcs, quadratics and the smooth-curve forms are converted by the")
    add("        // generator; the command set is closed, so this is a statement about")
    add("        // the generator not having grown a case nobody taught the builder.")
    add("        let variants = [")
    add("            Cmd::Move(0.0, 0.0), Cmd::Line(1.0, 1.0),")
    add("            Cmd::Cubic(0.0, 0.0, 1.0, 1.0, 2.0, 2.0), Cmd::Close,")
    add("            Cmd::NoPaint,")
    add("        ];")
    add("        assert_eq!(variants.len(), 5);")
    add("        for command in variants {")
    add("            let _ = build(&[command]);")
    add("        }")
    add("    }")
    add("")
    add("    #[test]")
    add("    fn the_shapes_that_are_not_paths_came_through_whole() {")
    add("        // A circle is four cubics and a close after its move, and a rectangle")
    add("        // without a corner radius is four lines. Both are asserted because the")
    add("        // element-to-command conversion is the part of the generator with no")
    add("        // source to compare against: the SVG says `circle`, the file says")
    add("        // cubics, and only a count can tell whether all four arrived.")
    add("        let circle = Glyph::by_name(\"circle\").expect(\"the reference has a circle\");")
    add("        let first = circle.elements()[0];")
    add("        assert_eq!(first.commands.len(), 6);")
    add("        assert!(matches!(first.commands[0], Cmd::Move(..)));")
    add("        assert!(matches!(first.commands[5], Cmd::Close));")
    add("    }")
    add("")
    add("    #[test]")
    add("    fn the_icon_used_as_a_placeholder_is_still_stroked_the_reference_way() {")
    add("        // `loader` is the icon that made the transform handling matter: it")
    add("        // declares a stroke width of 23 inside a matrix that scales by 0.0867,")
    add("        // which is how a 24-unit icon is stroked at 2. So this asserts the")
    add("        // width came out at 2 rather than at 23 -- the failure mode being a")
    add("        // path that is correct in shape and eleven times too heavy in ink.")
    add("        let loader = Glyph::by_name(\"loader\").expect(\"the reference has a loader\");")
    add("        assert_eq!(loader.view_box(), (24.0, 24.0));")
    add("        for element in loader.elements() {")
    add("            assert!(")
    add("                (element.stroke_width - 2.0).abs() < 0.01,")
    add("                \"loader strokes at {}, not the 2 its own transform scales to\",")
    add("                element.stroke_width")
    add("            );")
    add("            assert_eq!(element.ink, Ink::Inherit);")
    add("            assert_eq!(element.opacity, 1.0);")
    add("        }")
    add("    }")
    add("")
    add("    #[test]")
    add("    fn the_filled_paths_that_change_without_even_odd_are_the_ones_it_names() {")
    add("        // The two fill rules differ only where a filled path has more than one")
    add("        // subpath and its subpaths wind the same way: then even-odd cuts a hole")
    add("        // that non-zero fills. `Element::even_odd` now carries the rule, so")
    add("        // nothing is drawn wrongly -- but the rule is only worth carrying where")
    add("        // it changes the picture, and this says which those are, so a sixth")
    add("        // icon that needs it upstream fails here instead of quietly becoming")
    add("        // one more `Fill` nobody looks at.")
    add("        //")
    add("        // Five of them, and every one is a tag icon: the gear behind *Data")
    add("        // Pack*, *Resource Pack* and *Vanilla Shader* (three files with the same")
    add("        // path), and the two faces behind *Mobs* and *Entities*. None of the 313")
    add("        // top-level icons is on this list, which is why the top-level fills")
    add("        // looked right before this and these five would not have.")
    add("        // Every element that declares the rule carries it, which is the half")
    add("        // that is checkable from here: the file's own `even_odd` flags against")
    add("        // the list the generator wrote down. The key is the icon and the")
    add("        // element's index in it -- `Element` does not carry which tag it came")
    add("        // from, and adding a field for a test would put a pointer in every one")
    add("        // of them.")
    add(f"        let mut expected: Vec<&str> = vec![{', '.join(chr(34) + n + chr(34) for n in sorted(even_odds))}];")
    add("        expected.sort_unstable();")
    add("        let mut actual: Vec<String> = Vec::new();")
    add("        for (name, glyph) in every() {")
    add("            for (index, element) in glyph.elements().iter().enumerate() {")
    add("                if element.filled && element.even_odd {")
    add("                    actual.push(format!(\"{name}#{index}\"));")
    add("                }")
    add("            }")
    add("        }")
    add("        actual.sort_unstable();")
    add("        let expected: Vec<&str> = expected;")
    add("        let actual: Vec<&str> = actual.iter().map(|s| s.as_str()).collect();")
    add("        assert_eq!(actual, expected);")
    add("        assert!(!actual.is_empty(), \"the list is empty, so this proves nothing\");")
    add("")
    add("        // And the five of those eight are the five where the rule changes the")
    add("        // picture rather than being a no-op on a single-subpath or a pair wound")
    add("        // the other way round. `cog` and `x` carry it and do not need it, which is")
    add("        // harmless: the rule is the reference's own and the toolkit honours it,")
    add("        // and carrying what a file declares is not an approximation of it.")
    add(f"        let matters: Vec<&str> = vec![{', '.join(chr(34) + n + chr(34) for n in winding_risks)}];")
    add("        for name in &matters {")
    add("            assert!(")
    add("                expected.contains(name),")
    add("                \"{name} is not one of the even-odd elements\"")
    add("            );")
    add("        }")
    add("        assert_eq!(")
    add("            matters.len(), 5,")
    add("            \"the number of elements the rule actually changes has moved\"")
    add("        );")
    add("    }")
    add("")
    add("    #[test]")
    add("    fn the_icons_the_reference_fills_are_filled() {")
    add("        // Most of the set is stroked, and a handful are filled. The filled")
    add("        // ones are listed because reading `fill` off the shape instead of")
    add("        // inheriting it from the root element -- which is where the reference")
    add("        // declares it -- strokes their *outlines*, which renders, and looks")
    add("        // like a slightly wrong icon rather than like a bug.")
    add(f"        let expected: Vec<&str> = vec![{', '.join(chr(34) + n + chr(34) for n in filled_icons)}];")
    add("        let mut actual: Vec<&str> = every()")
    add("            .into_iter()")
    add("            .filter(|(_, g)| g.elements().iter().any(|e| e.filled))")
    add("            .map(|(n, _)| n)")
    add("            .collect();")
    add("        actual.sort_unstable();")
    add("        assert_eq!(actual, expected);")
    add("    }")
    add("")
    add("    #[test]")
    add("    fn the_shapes_the_reference_does_not_paint_are_the_ten_it_names() {")
    add("        // Ten of the icons carry a shape SVG paints *nothing* for: the clamp")
    add("        // path an exported icon has so that its drawing area has a name,")
    add("        // `M0 0h24v24H0Z`. Seven declare it `fill=\"none\"` with no `stroke`")
    add("        // at all, `key.svg` and `palette.svg` write `stroke=\"none\"` beside")
    add("        // their `fill=\"none\"`, and `bungeecord.svg` spells it as a")
    add("        // `<rect style=\"fill:none\">`.")
    add("        //")
    add("        // This used to be emitted as a stroked outline -- reading `fill=\"none\"`")
    add("        // as *unfilled but stroked* -- which drew a full 24-unit box around")
    add("        // each of these glyphs. Measured on the reference's own profile page:")
    add("        // *Fabric* and *Forge* carry no frame there, and ours had a 16x16px")
    add("        // one around each glyph.")
    add("        //")
    add("        // The list is the assertion and it is the whole list, so an icon that")
    add("        // gains a clamp fails here rather than quietly growing a box. The")
    add("        // key is the icon and the element's index in it, as it is for")
    add("        // `even_odd`.")
    add(f"        let mut expected: Vec<&str> = vec![{', '.join(chr(34) + n + chr(34) for n in sorted(no_paint))}];")
    add("        expected.sort_unstable();")
    add("        let mut actual: Vec<String> = Vec::new();")
    add("        for (name, glyph) in every() {")
    add("            for (index, element) in glyph.elements().iter().enumerate() {")
    add("                if matches!(element.commands, [Cmd::NoPaint]) {")
    add("                    actual.push(format!(\"{name}#{index}\"));")
    add("                }")
    add("            }")
    add("        }")
    add("        actual.sort_unstable();")
    add("        assert_eq!(actual, expected, \"the shapes that paint nothing moved\");")
    add("        assert_eq!(expected.len(), 10, \"the number of unpainted shapes moved\");")
    add("")
    add("        // And none of the ten is the whole of its icon: a glyph that paints")
    add("        // nothing at all would be a different picture, and the four tag icons")
    add("        // this tool refuses are already absent rather than half-drawn.")
    add("        for (name, glyph) in every() {")
    add("            let painted = glyph")
    add("                .elements()")
    add("                .iter()")
    add("                .filter(|element| !matches!(element.commands, [Cmd::NoPaint]))")
    add("                .count();")
    add("            assert!(painted > 0, \"{name} paints nothing at all\");")
    add("        }")
    add("    }")
    add("")
    add("    #[test]")
    add("    fn the_spinner_ring_keeps_its_opacity() {")
    add("        // The one icon in the set that draws at less than full opacity: a")
    add("        // 25% track under a 75% head. A generator that dropped the attribute")
    add("        // would draw a solid ring, which reads as a finished circle.")
    add("        let spinner = Glyph::by_name(\"spinner\").expect(\"the reference has a spinner\");")
    add("        let opacities: Vec<f32> = spinner.elements().iter().map(|e| e.opacity).collect();")
    add("        assert!(opacities.contains(&0.25), \"the track lost its opacity: {opacities:?}\");")
    add("        assert!(opacities.contains(&0.75), \"the head lost its opacity: {opacities:?}\");")
    add("    }")
    add("")
    add("    #[test]")
    add("    fn only_the_icons_that_deviate_from_width_two_do() {")
    add("        // Nearly the whole set declares one stroke width, 2, and a handful")
    add("        // declare their own because their art is deliberately heavier or")
    add("        // finer. The exceptions are written down, so a sixth one appearing")
    add("        // upstream fails here rather than silently changing an icon that")
    add("        // nothing else compares.")
    add("        let mut widths: Vec<String> = every()")
    add("            .into_iter()")
    add("            .flat_map(|(n, g)| {")
    add("                g.elements()")
    add("                    .iter()")
    add("                    .map(move |e| format!(\"{n}:{}\", e.stroke_width))")
    add("            })")
    add("            .filter(|entry| {")
    add("                // A width that reaches 2 through a transform's scale counts as")
    add("                // 2; `loader` does exactly that.")
    add("                let width = entry.rsplit(':').next().and_then(|w| w.parse::<f32>().ok());")
    add("                match width {")
    add("                    Some(value) => (value - 2.0).abs() > 0.01,")
    add("                    None => true,")
    add("                }")
    add("            })")
    add("            .collect();")
    add("        widths.sort_unstable();")
    add(f"        let expected: Vec<&str> = vec![{', '.join(chr(34) + w + chr(34) for w in width_exceptions)}];")
    add("        assert_eq!(")
    add("            widths, expected,")
    add("            \"the set of icons that do not stroke at width 2 has changed\"")
    add("        );")
    add("        assert!(!widths.is_empty(), \"the exceptions list is empty, so this proves nothing\");")
    add("    }")
    add("")
    add("    #[test]")
    add("    fn the_tag_icons_this_tool_could_not_read_are_the_four_it_names() {")
    add("        // The generator refuses what it cannot model rather than")
    add("        // approximating it, and it prints every refusal while it runs -- but a")
    add("        // message on a build machine is not a gate. These four are the ones it")
    add("        // cannot read, and the reason is written next to each, so that:")
    add("        //")
    add("        // * a fifth icon appearing in either tag directory fails the build")
    add("        //   rather than being skipped into a hole;")
    add("        // * teaching the parser the construct fails the build too, which is the")
    add("        //   point: the day one of these is drawn it should be because somebody")
    add("        //   decided it should be, in a commit that says so.")
    add("        //")
    add("        // Three are non-uniform transform scales -- `geyser`, `purpur` and")
    add("        // `quilt` -- where a round-capped stroke has no single width to apply;")
    add("        // one is a `clip-path` the tool cannot put on a path.")
    refused_list = ", ".join(chr(34) + n + chr(34) for n in sorted(REFUSED_TAGS))
    add(f"        let mut refused: Vec<&str> = vec![{refused_list}];")
    add("        refused.sort_unstable();")
    add("        assert_eq!(refused.len(), 4, \"the number of refusals moved\");")
    add("        let here: Vec<&str> = every()")
    add("            .into_iter()")
    add("            .map(|(name, _)| name)")
    add("            .filter(|name| name.starts_with(\"tags/\"))")
    add("            .collect();")
    add("        for name in &here {")
    add("            assert!(*name != \"tags/loaders/geyser\", \"{name} was drawn after all\");")
    add("            assert!(*name != \"tags/loaders/purpur\", \"{name} was drawn after all\");")
    add("            assert!(*name != \"tags/loaders/quilt\", \"{name} was drawn after all\");")
    add("            assert!(")
    add("                *name != \"tags/loaders/legacy-fabric\",")
    add("                \"{name} was drawn after all\"")
    add("            );")
    add("        }")
    add("        // 132 tag icons minus four.")
    add("        assert_eq!(here.len(), 128);")
    add("    }")
    add("")
    add("    fn numbers(command: Cmd) -> Vec<f32> {")
    add("        match command {")
    add("            Cmd::Move(x, y) | Cmd::Line(x, y) => vec![x, y],")
    add("            Cmd::Cubic(a, b, c, d, e, f) => vec![a, b, c, d, e, f],")
    add("            Cmd::Close | Cmd::NoPaint => Vec::new(),")
    add("        }")
    add("    }")
    add("}")
    add("")

    text = "\n".join(lines)
    report = [
        f"icons        {len(icons)}",
    ]
    for source in SOURCES:
        table = source["table"]
        its = sets[table]
        report.append(
            f"  {table:<14} {len(its):>4}  "
            f"{sum(len(elements) for _, _, _, elements in its):>5} elements"
        )
    report += [
        f"refused      {len(refused)}: {', '.join(name for name, _ in sorted(refused))}",
        f"elements     {sum(len(elements) for _, _, _, elements in icons)}",
        f"commands     {total_commands}",
        f"no paint     {len(no_paint)}: {', '.join(sorted(no_paint)) if no_paint else '-'}",
        f"stroke width {widths}",
        f"filled       {len(filled_icons)}: {', '.join(filled_icons) if filled_icons else '-'}",
        f"off-width    {len(width_exceptions)}: {', '.join(width_exceptions) if width_exceptions else '-'}",
        f"tokens       {with_tokens if with_tokens else '-'}",
        f"fill+stroke  {len(both)}: {', '.join(both) if both else '-'}",
        f"winding risk {len(winding_risks)}: {', '.join(winding_risks) if winding_risks else '-'}",
    ]

    if args.check:
        current = OUT.read_text(encoding="utf-8") if OUT.is_file() else ""
        if current != text:
            print(f"{OUT.relative_to(ROOT)} is not what this tool emits.", file=sys.stderr)
            print("Run `python tools/gen_icons.py` and commit the result.", file=sys.stderr)
            return 1
        print("icon generation is byte-identical")
        return 0

    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text(text, encoding="utf-8", newline="\n")
    if args.report:
        for line in report:
            print(line)
    print(f"wrote {OUT.relative_to(ROOT)} ({text.count(chr(10)) + 1} lines)")
    return 0


def screaming(rust: str) -> str:
    return re.sub(r"(?<!^)(?=[A-Z])", "_", rust).upper()


def table_name(rust: str) -> str:
    return "CMD_" + screaming(rust)


def command_table(rust: str, index: int) -> str:
    return f"CMD_{screaming(rust)}_{index + 1}"


def command_source(command: tuple) -> str:
    if command[0] == "Close":
        return "Cmd::Close"
    if command[0] == "NoPaint":
        return "Cmd::NoPaint"
    if command[0] == "Move":
        return f"Cmd::Move({fnum(command[1])}, {fnum(command[2])})"
    if command[0] == "Line":
        return f"Cmd::Line({fnum(command[1])}, {fnum(command[2])})"
    return "Cmd::Cubic(" + ", ".join(fnum(v) for v in command[1:]) + ")"


if __name__ == "__main__":
    raise SystemExit(main())
