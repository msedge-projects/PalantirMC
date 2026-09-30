#!/usr/bin/env python3
"""Compile the reference client's icon set into geometry a native window can stroke.

The shell's icons are PNGs carved out of another launcher's binary. The reference
keeps 313 SVG files, and they are outlines rather than pictures: 305 of them are
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
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
VENDOR = ROOT / "vendor" / "modrinth-app"
ICONS = VENDOR / "assets" / "icons"
OUT = ROOT / "crates" / "palantir-desktop" / "src" / "icons_gen.rs"

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
                 ink: tuple[str, str | None], element: str, opacity: float = 1.0):
        self.commands = commands
        self.stroke_width = stroke_width
        self.filled = filled
        self.ink = ink
        self.element = element
        self.opacity = opacity


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


def read_icon(path: Path, both: list[str],
              winding_risks: list[str]) -> tuple[tuple[float, float], list[Element]]:
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
            both.append(f"{path.name}:{tag}")
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
        if has_fill and inherited.get("fill-rule", "nonzero") == "evenodd":
            areas = subpath_areas(commands)
            if len(areas) > 1 and all(a > 0 for a in areas) or len(areas) > 1 and all(a < 0 for a in areas):
                winding_risks.append(f"{path.name}:{tag}")

        if has_fill:
            elements.append(Element(commands, width, True, ink_hint(fill), tag, opacity))
        if has_stroke or not has_fill:
            stroke_ink = stroke if has_stroke else None
            elements.append(Element(commands, width, False, ink_hint(stroke_ink), tag, opacity))
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

    files = sorted(ICONS.glob("*.svg"))
    icons = []
    both: list[str] = []
    winding_risks: list[str] = []
    for path in files:
        name = path.stem
        try:
            box, elements = read_icon(path, both, winding_risks)
        except Unsupported as exc:
            print(f"{path.name}: {exc}", file=sys.stderr)
            return 1
        icons.append((name, variant(name), box, elements))

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
    add("//! These are the Modrinth App's own icons, out of the 313 SVGs in")
    add("//! `vendor/modrinth-app/assets/icons`, and they are outlines rather than")
    add("//! pictures: the reference draws them stroked in `currentColor` at width 2")
    add("//! with round caps and joins, which is what lets an icon take the colour of")
    add("//! the control it sits in. This is why they are geometry and not a bitmap --")
    add("//! a rasterised icon cannot be tinted, and the shell's previous icons were")
    add("//! bitmaps carved out of another launcher's binary.")
    add("//!")
    add("//! Arcs, quadratics and the smooth-curve commands are all already converted")
    add("//! to cubics by the generator, and `<g transform>` is applied to every point")
    add("//! before it is written: an affine map commutes with Bezier evaluation, so")
    add("//! transforming the control points transforms the curve exactly.")
    add("//!")
    add("//! Gate: `python tools/gen_icons.py --check` regenerates and compares bytes.")
    add("")
    add("// Nothing draws these yet: the shell that will is stage 2 of")
    add("// `docs/superpowers/specs/2026-09-24-modrinth-native-rewrite.md`, and the shell")
    add("// on screen today draws `crate::icons`' carved PNGs. Delete this line the day")
    add("// the last icon is drawn from here.")
    add("#![allow(dead_code)]")
    add("")
    add("use iced::widget::canvas::{LineCap, LineDash, LineJoin, Path, Stroke, Style};")
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
    add("    /// The element's own `opacity`, which multiplies whatever ink it gets.")
    add("    ///")
    add("    /// The spinner's ring is a quarter opaque, so this is not decoration:")
    add("    /// a stroke at full opacity there draws a solid ring instead of a track.")
    add("    pub opacity: f32,")
    add("}")
    add("")
    add("/// Every icon in the reference's set.")
    add("#[derive(Clone, Copy, Debug, PartialEq, Eq)]")
    add("pub enum Glyph {")
    for name, rust, _, _ in icons:
        add(f"    /// `{name}.svg`")
        add(f"    {rust},")
    add("}")
    add("")
    add("/// The reference's set, in name order, with the file name beside each.")
    add("pub const ALL: &[(&str, Glyph)] = &[")
    for name, rust, _, _ in icons:
        add(f"    (\"{name}\", Glyph::{rust}),")
    add("];")
    add("")
    add("impl Glyph {")
    add("    /// The reference's own file name, without the extension.")
    add("    pub fn name(self) -> &'static str {")
    add("        ALL.iter().find(|(_, g)| *g == self).map(|(n, _)| *n).unwrap_or(\"\")")
    add("    }")
    add("")
    add("    /// The icon a name refers to, for a gate that walks the reference.")
    add("    pub fn by_name(name: &str) -> Option<Glyph> {")
    add("        ALL.iter().find(|(n, _)| *n == name).map(|(_, g)| *g)")
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
    add("            }")
    add("        }")
    add("    })")
    add("}")
    add("")
    add("/// How one element is painted: stroked in the ink, or filled with it.")
    add("///")
    add("/// The reference fills a handful of its 313 icons and strokes the rest, so")
    add("/// this is a property of the element rather than of the icon.")
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
    add("    /// Fill the path.")
    add("    Fill(Color),")
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
    add("            Paint::Fill(ink)")
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
        f"{name}:{element.stroke_width:g}"
        for name, _, _, elements in icons
        for element in elements
        if abs(element.stroke_width - 2.0) > 0.01
    )
    winding_risks = sorted(set(winding_risks))
    with_tokens = sorted({(name, e.ink[1]) for name, _, _, els in icons for e in els if e.ink[0] == "Token"})
    filled_icons = sorted({name for name, _, _, els in icons if any(e.filled for e in els)})
    widths = sorted({e.stroke_width for _, _, _, els in icons for e in els})

    add("#[cfg(test)]")
    add("mod tests {")
    add("    use super::*;")
    add("")
    add("    #[test]")
    add("    fn the_whole_reference_is_here() {")
    add(f"        // The vendored tree holds {len(icons)} SVGs; a generator that quietly")
    add("        // skipped one that it could not parse would leave a hole nothing else")
    add("        // notices, so the count is asserted rather than the absence of errors.")
    add(f"        assert_eq!(ALL.len(), {len(icons)});")
    add("        let mut names: Vec<&str> = ALL.iter().map(|(n, _)| *n).collect();")
    add("        names.sort_unstable();")
    add("        let mut unique = names.clone();")
    add("        unique.dedup();")
    add("        assert_eq!(names, unique, \"two icons share a name\");")
    add("    }")
    add("")
    add("    #[test]")
    add("    fn every_icon_draws_something_finite() {")
    add("        for (name, glyph) in ALL {")
    add("            let elements = glyph.elements();")
    add("            assert!(!elements.is_empty(), \"{name} has no elements\");")
    add("            for element in elements {")
    add("                assert!(!element.commands.is_empty(), \"{name} has an empty element\");")
    add("                assert!(")
    add("                    matches!(element.commands[0], Cmd::Move(..)),")
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
    add("        ];")
    add("        assert_eq!(variants.len(), 4);")
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
    add("    fn no_filled_path_needs_a_winding_rule_the_toolkit_cannot_give_it() {")
    add("        // The reference fills with even-odd; the toolkit fills with non-zero.")
    add("        // They differ only when a filled path has more than one subpath and its")
    add("        // subpaths wind the same way -- then a hole the reference cuts comes out")
    add("        // solid here. Of the filled paths in this set, six have more than one")
    add("        // subpath and four of those wind their subpaths oppositely, so they cannot")
    add("        // differ; the remaining two do not declare even-odd at all, so non-zero is")
    add("        // the rule they are drawn with upstream. This asserts that reading, so an")
    add("        // icon added upstream that *does* need it fails here instead of rendering")
    add("        // as a solid blob nobody compares. The list below is what the generator")
    add("        // found by winding every subpath; the assertion is that it found nothing.")
    add(f"        let mut risky: Vec<&str> = vec![{', '.join(chr(34) + n + chr(34) for n in winding_risks)}];")
    add("        risky.sort_unstable();")
    add("        assert!(")
    add("            risky.is_empty(),")
    add("            \"a filled path now needs even-odd winding: {risky:?}\"")
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
    add("        let mut actual: Vec<&str> = ALL")
    add("            .iter()")
    add("            .filter(|(_, g)| g.elements().iter().any(|e| e.filled))")
    add("            .map(|(n, _)| *n)")
    add("            .collect();")
    add("        actual.sort_unstable();")
    add("        assert_eq!(actual, expected);")
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
    add("        let mut widths: Vec<String> = ALL")
    add("            .iter()")
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
    add("    fn numbers(command: Cmd) -> Vec<f32> {")
    add("        match command {")
    add("            Cmd::Move(x, y) | Cmd::Line(x, y) => vec![x, y],")
    add("            Cmd::Cubic(a, b, c, d, e, f) => vec![a, b, c, d, e, f],")
    add("            Cmd::Close => Vec::new(),")
    add("        }")
    add("    }")
    add("}")
    add("")

    text = "\n".join(lines)
    report = [
        f"icons        {len(icons)}",
        f"elements     {sum(len(elements) for _, _, _, elements in icons)}",
        f"commands     {total_commands}",
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
    if command[0] == "Move":
        return f"Cmd::Move({fnum(command[1])}, {fnum(command[2])})"
    if command[0] == "Line":
        return f"Cmd::Line({fnum(command[1])}, {fnum(command[2])})"
    return "Cmd::Cubic(" + ", ".join(fnum(v) for v in command[1:]) + ")"


if __name__ == "__main__":
    raise SystemExit(main())
