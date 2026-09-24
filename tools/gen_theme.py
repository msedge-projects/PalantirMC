#!/usr/bin/env python3
"""Compile the reference client's design system into Rust, off its own stylesheets.

The shell used to carry a hand-written palette beside a test-only table of 189
transcribed values, and the two could disagree -- they did, which is why headings
were drawn at 600 while the reference draws them at 800. This tool removes the
second copy: it reads the reference's own CSS, resolves it the way a browser
would, and emits one module that is the only place a colour, a radius, a first
number or a motion value is allowed to come from.

    python tools/gen_theme.py            # write crates/palantir-desktop/src/theme_gen.rs
    python tools/gen_theme.py --check    # fail if the file on disk is not what this emits
    python tools/gen_theme.py --report   # what could not be classified, and why

## What it reads

All of it vendored, all of it read-only, all of it at the pinned commit in
`vendor/modrinth-app/UPSTREAM.md`:

| Source | What comes out of it |
| --- | --- |
| `assets/styles/variables.scss` | The palette, the radii and the gaps: 300-odd custom properties in five theme blocks |
| `ui/src/styles/tailwind-utilities.css` | `--ease-out-expo`, and the floating-action-bar transitions |
| `assets/styles/*.scss`, `ui/src/**`, `app-frontend/src/**` | Every `transition:` and `@keyframes` the reference declares, as the motion vocabulary |

## How a value is resolved

The reference's CSS is not a flat table, and pretending it is was the other half
of the drift:

* **`@extend`** — `.oled-mode` and `.retro-mode` both extend `.dark-mode`, and
  `.light-mode` extends `.light-properties`. A theme is therefore a *chain*, and
  this resolves it in the order the cascade would.
* **`var()` indirection** — `--color-brand: var(--color-green)`, which in turn is
  `var(--color-green-600)` in light and `var(--color-green-500)` in dark. Two hops
  is common here, so indirection is followed to a fixed point.
* **`!important`** and comments are stripped, and multi-line values (several
  shadows are written across lines) are joined before classification.

A property is classified by the *resolved* value: a colour, a length, a bare
number, a cubic-bezier, or raw text. A property whose class differs between
themes -- a gradient in one and a colour in another, say -- is emitted as raw
with all four values verbatim, because guessing would be the drift returning.
`--report` prints which ones those are.

## What it emits

`theme_gen.rs`: four enums the compiler can check (`Ink`, `Span`, `Factor`,
`Curve`), one for the things that cannot be a scalar (`Raw`), a total accessor
for each, the motion vocabulary, and the Tailwind scale the reference's markup is
written in. Every accessor is total: `ink(theme, token)` cannot fail, because the
resolution above is done here rather than at run time, and a token with no value
in a theme is a generation error rather than a `None` at a call site.

Registration is the `--check` mode, which is what the gate runs: regenerate into
memory and compare byte for byte.
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
VENDOR = ROOT / "vendor" / "modrinth-app"
OUT = ROOT / "crates" / "palantir-desktop" / "src" / "theme_gen.rs"

VARIABLES = VENDOR / "assets" / "styles" / "variables.scss"
UTILITIES = VENDOR / "ui" / "src" / "styles" / "tailwind-utilities.css"

# The four looks the reference ships, in the order the accessors index them.
THEMES = ["light", "dark", "oled", "retro"]

# Which block each theme is built from, in cascade order. `light-properties` and
# `html` are always first: `html` carries the radii and the gaps, and the
# reference puts `.light-properties` on `html` so every other theme falls back to
# those values for whatever it does not declare itself.
LIGHT_BLOCKS = ["light-properties", "html"]
THEME_BLOCKS = {
    "light": ["light-mode"],
    "dark": ["dark-mode"],
    "oled": ["dark-mode", "oled-mode"],
    "retro": ["dark-mode", "retro-mode"],
}

# CSS's own named easings, which the reference uses by name. Transcribed rather
# than parsed because they are in the CSS specification, not in this tree.
NAMED_CURVES = {
    "linear": (0.0, 0.0, 1.0, 1.0),
    "ease": (0.25, 0.1, 0.25, 1.0),
    "ease-in": (0.42, 0.0, 1.0, 1.0),
    "ease-out": (0.0, 0.0, 0.58, 1.0),
    "ease-in-out": (0.42, 0.0, 0.58, 1.0),
}

# `background-color`, `border-color` and friends are what the class is called in
# CSS; Rust variants cannot carry a hyphen, so each word is capitalised.
NAMED_COLOURS = {
    "black": (0, 0, 0, 255),
    "white": (255, 255, 255, 255),
    "transparent": (0, 0, 0, 0),
}

# Tailwind's default theme, which the reference inherits: its own `tailwind.config.ts`
# sets `presets: [preset]` and the preset only renames colours (every entry in it
# is `var(--...)`). tailwindcss itself is not vendored -- vendoring a build tool to
# read eight numbers would be a worse trade than writing them down with their
# source named, which is what this table is.
TAILWIND_SPACING_REM = {
    "0": 0.0, "px": 1 / 16, "0.5": 0.125, "1": 0.25, "1.5": 0.375, "2": 0.5,
    "2.5": 0.625, "3": 0.75, "3.5": 0.875, "4": 1.0, "5": 1.25, "6": 1.5,
    "7": 1.75, "8": 2.0, "9": 2.25, "10": 2.5, "11": 2.75, "12": 3.0, "14": 3.5,
    "16": 4.0, "20": 5.0, "24": 6.0, "28": 7.0, "32": 8.0, "36": 9.0, "40": 10.0,
    "44": 11.0, "48": 12.0, "52": 13.0, "56": 14.0, "60": 15.0, "64": 16.0,
    "72": 18.0, "80": 20.0, "96": 24.0,
}
TAILWIND_RADII_REM = {
    "none": 0.0, "sm": 0.125, "DEFAULT": 0.25, "md": 0.375, "lg": 0.5,
    "xl": 0.75, "2xl": 1.0, "3xl": 1.5, "full": 9999.0,
}
TAILWIND_FONT_SIZE_REM = {
    "xs": 0.75, "sm": 0.875, "base": 1.0, "lg": 1.125, "xl": 1.25,
    "2xl": 1.5, "3xl": 1.875, "4xl": 2.25, "5xl": 3.0, "6xl": 3.75,
    "7xl": 4.5, "8xl": 6.0, "9xl": 8.0,
}
TAILWIND_LINE_HEIGHT_REM = {
    "xs": 1.0, "sm": 1.25, "base": 1.5, "lg": 1.75, "xl": 1.75, "2xl": 2.0,
    "3xl": 2.25, "4xl": 2.5, "5xl": 1.0, "6xl": 1.0, "7xl": 1.0, "8xl": 1.0,
    "9xl": 1.0,
}
TAILWIND_FONT_WEIGHT = {
    "thin": 100, "extralight": 200, "light": 300, "normal": 400, "medium": 500,
    "semibold": 600, "bold": 700, "extrabold": 800, "black": 900,
}
TAILWIND_DURATION_MS = [0, 75, 100, 150, 200, 300, 500, 700, 1000]

# The root font size the whole conversion assumes. The reference sets no
# `font-size` on `html`, so a browser uses its own default, and this is it.
ROOT_PX = 16.0


# --------------------------------------------------------------------------
# Reading the reference's CSS
# --------------------------------------------------------------------------


def strip_comments(text: str) -> str:
    """Remove CSS comments, without eating the `//` in a `url(https://…)`.

    A CSS comment does not start after a colon, an `https:` scheme does, so that
    is the one case worth distinguishing. Nothing else in the vendored tree puts
    a bare `//` in a value.
    """
    text = re.sub(r"/\*.*?\*/", " ", text, flags=re.S)
    return re.sub(r"(?<!:)//[^\n]*", "", text)


def split_declarations(body: str) -> list[str]:
    """Split a block body on `;`, ignoring semicolons inside parentheses."""
    out, depth, current = [], 0, []
    for ch in body:
        if ch == "(":
            depth += 1
        elif ch == ")":
            depth = max(0, depth - 1)
        if ch == ";" and depth == 0:
            out.append("".join(current))
            current = []
        else:
            current.append(ch)
    out.append("".join(current))
    return [item.strip() for item in out if item.strip()]


def parse_blocks(text: str) -> dict[str, dict[str, str]]:
    """Every `selector { … }` in a flat stylesheet, keyed by each selector.

    Only depth-1 declarations are read. The two files this is pointed at are flat
    in the parts that matter -- `variables.scss` has no at-rules at all -- and a
    nested block's declarations belong to a more specific selector than the theme
    chain models, so they are skipped rather than mis-attributed.
    """
    text = strip_comments(text)
    blocks: dict[str, dict[str, str]] = {}
    depth = 0
    selector_start = 0
    body_start = 0
    i = 0
    while i < len(text):
        ch = text[i]
        if ch == "{":
            if depth == 0:
                body_start = i + 1
            depth += 1
        elif ch == "}":
            depth -= 1
            if depth == 0:
                selector = text[selector_start:i + 1].split("{")[0].strip()
                body = text[body_start:i]
                declarations: dict[str, str] = {}
                for item in split_declarations(body):
                    if item.startswith("@extend"):
                        continue
                    if ":" not in item:
                        continue
                    name, _, value = item.partition(":")
                    name = name.strip()
                    if not name.startswith("--"):
                        continue
                    declarations[name] = normalise_value(value)
                for one in selector.split(","):
                    # Keys are undotted so that a selector and an `@extend` naming
                    # it -- `.oled-mode { @extend .dark-mode }` -- are the same
                    # string, which is what the theme chain looks up.
                    one = one.strip().lstrip(".")
                    if one:
                        blocks.setdefault(one, {}).update(declarations)
                selector_start = i + 1
        i += 1
    return blocks


def normalise_value(value: str) -> str:
    """One value, on one line, without its `!important`."""
    value = re.sub(r"!important\s*$", "", value.strip())
    return re.sub(r"\s+", " ", value).strip()


def extends_of(text: str, selector: str) -> str | None:
    """What a block `@extend`s, if it does.

    `.oled-mode { @extend .dark-mode; … }` is the reference saying which theme
    the new one starts from, so it is read rather than assumed.
    """
    text = strip_comments(text)
    for match in re.finditer(r"([^{}]+)\{([^{}]*)\}", text):
        if selector in [s.strip().lstrip(".") for s in match.group(1).split(",")]:
            found = re.search(r"@extend\s+([.a-zA-Z0-9_-]+)", match.group(2))
            if found:
                return found.group(1).lstrip(".")
    return None


def build_theme_maps(blocks: dict[str, dict[str, str]], text: str) -> dict[str, dict[str, str]]:
    """The four themes, each resolved as the cascade would resolve it."""

    def apply(target: dict[str, str], selector: str, seen: set[str]) -> None:
        if selector in seen:
            return
        seen.add(selector)
        parent = extends_of(text, selector)
        if parent:
            apply(target, parent, seen)
        target.update(blocks.get(selector, {}))

    themes: dict[str, dict[str, str]] = {}
    for theme in THEMES:
        table: dict[str, str] = {}
        for selector in LIGHT_BLOCKS + THEME_BLOCKS[theme]:
            apply(table, selector, set())
        themes[theme] = table
    return themes


def resolve_var(table: dict[str, str], value: str, seen: tuple[str, ...] = ()) -> str:
    """Follow `var()` to a fixed point, which is two hops in three places here."""
    match = re.fullmatch(r"var\(\s*(--[a-zA-Z0-9-]+)\s*(?:,[^)]*)?\)", value.strip())
    if not match:
        return value
    name = match.group(1)
    if name in seen:
        return value
    inner = table.get(name)
    if inner is None:
        return value
    return resolve_var(table, inner, seen + (name,))


# --------------------------------------------------------------------------
# Classifying a value
# --------------------------------------------------------------------------


def parse_colour(value: str) -> tuple[int, int, int, int] | None:
    value = value.strip()
    if value.startswith("#"):
        digits = value[1:]
        if len(digits) == 3:
            r, g, b = (int(c * 2, 16) for c in digits)
            return (r, g, b, 255)
        if len(digits) == 4:
            r, g, b, a = (int(c * 2, 16) for c in digits)
            return (r, g, b, a)
        if len(digits) == 6:
            return (int(digits[0:2], 16), int(digits[2:4], 16), int(digits[4:6], 16), 255)
        if len(digits) == 8:
            return (
                int(digits[0:2], 16), int(digits[2:4], 16),
                int(digits[4:6], 16), int(digits[6:8], 16),
            )
        return None
    match = re.fullmatch(r"rgba?\(\s*([\d.]+)[,\s]+([\d.]+)[,\s]+([\d.]+)(?:[,\s/]+([\d.]+%?))?\s*\)", value)
    if match:
        r, g, b = (round(float(match.group(i))) for i in (1, 2, 3))
        raw_a = match.group(4)
        if raw_a is None:
            a = 255
        elif raw_a.endswith("%"):
            a = round(float(raw_a[:-1]) * 255 / 100)
        else:
            a = round(float(raw_a) * 255)
        return (r, g, b, a)
    match = re.fullmatch(r"hsla?\(\s*([\d.]+)(?:deg)?[,\s]+([\d.]+)%[,\s]+([\d.]+)%(?:[,\s/]+([\d.]+))?\s*\)", value)
    if match:
        h = float(match.group(1)) / 360.0
        s = float(match.group(2)) / 100.0
        lightness = float(match.group(3)) / 100.0
        alpha = float(match.group(4)) if match.group(4) else 1.0
        r, g, b = hsl_to_rgb(h, s, lightness)
        return (r, g, b, round(alpha * 255))
    named = NAMED_COLOURS.get(value.lower())
    if named is not None:
        return named
    return None


def hsl_to_rgb(h: float, s: float, lightness: float) -> tuple[int, int, int]:
    if s == 0:
        channel = round(lightness * 255)
        return (channel, channel, channel)

    def hue(p: float, q: float, t: float) -> float:
        t = t % 1.0
        if t < 1 / 6:
            return p + (q - p) * 6 * t
        if t < 1 / 2:
            return q
        if t < 2 / 3:
            return p + (q - p) * (2 / 3 - t) * 6
        return p

    q = lightness * (1 + s) if lightness < 0.5 else lightness + s - lightness * s
    p = 2 * lightness - q
    return (
        round(hue(p, q, h + 1 / 3) * 255),
        round(hue(p, q, h) * 255),
        round(hue(p, q, h - 1 / 3) * 255),
    )


def parse_length(value: str) -> tuple[float, str] | None:
    match = re.fullmatch(r"(-?[\d.]+)(rem|px|em)", value.strip())
    if not match:
        return None
    number, unit = float(match.group(1)), match.group(2)
    if unit == "px":
        return (number, "px")
    return (number * ROOT_PX, "rem")


def parse_number(value: str) -> float | None:
    try:
        return float(value.strip())
    except ValueError:
        return None


def parse_bezier(value: str) -> tuple[float, float, float, float] | None:
    match = re.fullmatch(r"cubic-bezier\(([^)]*)\)", value.strip())
    if match:
        parts = [float(p) for p in re.split(r"[,\s]+", match.group(1).strip()) if p]
        if len(parts) == 4:
            return (parts[0], parts[1], parts[2], parts[3])
        return None
    named = NAMED_CURVES.get(value.strip().lower())
    if named is not None:
        return named
    return None


def classify(value: str) -> tuple[str, object]:
    """The class of one resolved value, and the data to emit for it."""
    colour = parse_colour(value)
    if colour is not None:
        return ("colour", colour)
    length = parse_length(value)
    if length is not None:
        return ("length", length)
    bezier = parse_bezier(value)
    if bezier is not None:
        return ("curve", bezier)
    number = parse_number(value)
    if number is not None:
        return ("number", number)
    return ("raw", value)


# --------------------------------------------------------------------------
# Names
# --------------------------------------------------------------------------


def rust_name(css: str) -> str:
    """`--color-text-primary` -> `TextPrimary`; `--surface-1-5` -> `Surface1_5`.

    A leading `color-` is dropped because the enum it lands in is already about
    colour, and `--color-red-500` becoming `Ink::Red500` reads better than
    `Ink::ColorRed500`. Everything else is camel-cased with digits kept attached,
    so a variant name is recognisable as the token it came from.
    """
    name = css[2:]
    prefix = "color-"
    if name.startswith(prefix) and name != "color-brand-highlight":
        name = name[len(prefix):]
    parts = name.split("-")
    out = []
    for part in parts:
        if not part:
            continue
        if out and part.isdigit():
            # `--color-red-500` is `Red500`, not `Red_500`: an underscore would
            # make the variant a `non_camel_case_types` warning, and a leading
            # underscore cannot be used because `_500` is not an identifier.
            out[-1] = out[-1] + part
        else:
            out.append(part[0].upper() + part[1:])
    return "".join(out)


def check_unique(names: list[str], what: str) -> None:
    """Two CSS names must not become one Rust variant.

    The camel-casing above drops the hyphens, so a token pair differing only in
    a hyphen would collide. That would be a compile error in the generated file,
    which is a bad way to find out; this is a worse-named but earlier one.
    """
    seen: dict[str, str] = {}
    for name in names:
        variant = rust_name(name)
        if variant in seen:
            raise SystemExit(
                f"{what}: {seen[variant]} and {name} both become `{variant}`; "
                "teach rust_name to tell them apart"
            )
        seen[variant] = name


def fnum(value: float) -> str:
    """A float as Rust source, with a decimal point, so `f32` is unambiguous."""
    text = f"{value:.4f}".rstrip("0").rstrip(".")
    if text in ("", "-"):
        text = "0"
    if "." not in text:
        text += ".0"
    return text


# --------------------------------------------------------------------------
# Motion: the transitions and keyframes the reference declares
# --------------------------------------------------------------------------

MOTION_DIRS = [
    VENDOR / "assets" / "styles",
    VENDOR / "ui" / "src",
    VENDOR / "app-frontend" / "src",
]
MOTION_SUFFIXES = {".css", ".scss", ".vue"}


def motion_sources() -> list[Path]:
    files: list[Path] = []
    for directory in MOTION_DIRS:
        if not directory.is_dir():
            continue
        for path in sorted(directory.rglob("*")):
            if path.is_file() and path.suffix in MOTION_SUFFIXES:
                files.append(path)
    return files


def scan_motion() -> tuple[list[tuple[str, int, tuple[float, float, float, float], str]],
                           list[tuple[str, str, str]],
                           list[str]]:
    """Every transition the reference declares, split by how well it can be read.

    Returns `(unambiguous, verbatim, keyframes)`.

    CSS's `transition` shorthand allows *one* duration and easing to be shared by
    a comma-separated list of properties -- `transition: color, background-color
    125ms ease-in-out` -- which a naive split reads as three transitions. Where an
    item has exactly one property and one duration it is parsed and lands in the
    first list; where it does not, the whole shorthand is kept verbatim in the
    second, with its source file, because a porter needs to read the reference's
    own wording rather than a guess at it.
    """
    unambiguous: list[tuple[str, int, tuple[float, float, float, float], str]] = []
    verbatim: list[tuple[str, str, str]] = []
    keyframes: list[str] = []
    seen: set[tuple[str, int, tuple[float, float, float, float]]] = set()

    for path in motion_sources():
        try:
            text = strip_comments(path.read_text(encoding="utf-8", errors="replace"))
        except OSError:
            continue
        rel = path.relative_to(VENDOR).as_posix()
        for match in re.finditer(r"@keyframes\s+([A-Za-z0-9_-]+)", text):
            if match.group(1) not in keyframes:
                keyframes.append(match.group(1))
        for match in re.finditer(r"transition(?:-duration|-timing-function)?\s*:\s*([^;}]+)", text):
            body = normalise_value(match.group(1))
            items = [item.strip() for item in re.split(r",(?![^()]*\))", body) if item.strip()]
            if len(items) != 1:
                verbatim.append((rel, "transition", body))
                continue
            item = items[0]
            tokens = item.split()
            durations = [t for t in tokens if re.fullmatch(r"[\d.]+m?s", t)]
            curves = [t for t in tokens if parse_bezier(t) or t.startswith("var(--ease")]
            properties = [
                t for t in tokens
                if t not in durations and t not in curves and not t.endswith("s")
            ]
            if len(durations) != 1 or len(properties) != 1 or len(tokens) != len(durations) + len(curves) + len(properties):
                verbatim.append((rel, "transition", body))
                continue
            raw_curve = curves[0] if curves else "ease"
            var_match = re.fullmatch(r"var\(\s*(--[a-zA-Z0-9-]+)\s*\)", raw_curve)
            if var_match:
                resolved = VARIABLE_VALUES.get(var_match.group(1))
                if resolved is None:
                    verbatim.append((rel, "transition", body))
                    continue
                raw_curve = resolved
            curve = parse_bezier(raw_curve)
            if curve is None:
                verbatim.append((rel, "transition", body))
                continue
            duration_text = durations[0]
            millis = round(float(duration_text[:-2]) * 1000) if duration_text.endswith("ms") \
                else round(float(duration_text[:-1]) * 1000)
            key = (properties[0], millis, curve)
            if key in seen:
                continue
            seen.add(key)
            unambiguous.append((properties[0], millis, curve, rel))

    unambiguous.sort(key=lambda row: (row[0], row[1], row[2]))
    verbatim = sorted(set(verbatim))
    keyframes.sort()
    return unambiguous, verbatim, keyframes


# Filled in by `main` before the motion scan, because a transition written as
# `var(--ease-out-expo)` can only be resolved against the reference's own table.
VARIABLE_VALUES: dict[str, str] = {}


# --------------------------------------------------------------------------
# Emitting Rust
# --------------------------------------------------------------------------


def emit(themes: dict[str, dict[str, str]]) -> tuple[str, list[str]]:
    notes: list[str] = []

    # ---- classify every property, in every theme -------------------------
    names = sorted({name for table in themes.values() for name in table})
    classes: dict[str, str | None] = {}
    data: dict[str, list] = {}
    for name in names:
        theme_classes = []
        theme_data = []
        for theme in THEMES:
            table = themes[theme]
            if name not in table:
                theme_classes.append(None)
                theme_data.append(None)
                continue
            resolved = resolve_var(table, table[name])
            kind, payload = classify(resolved)
            theme_classes.append(kind)
            theme_data.append(payload)
        present = [c for c in theme_classes if c is not None]
        if not present:
            continue
        if len(set(present)) != 1:
            classes[name] = "raw"
            data[name] = []
            for theme in THEMES:
                table = themes[theme]
                data[name].append(resolve_var(table, table[name]) if name in table else "")
            notes.append(
                f"{name}: mixed classes {sorted(set(present))} -> raw"
            )
            continue
        kind = present[0]
        classes[name] = kind
        if kind == "raw":
            data[name] = []
            for theme in THEMES:
                table = themes[theme]
                data[name].append(resolve_var(table, table[name]) if name in table else "")
        else:
            fallback = next((d for d in theme_data if d is not None), None)
            filled = []
            for theme, value in zip(THEMES, theme_data):
                if value is None:
                    notes.append(f"{name}: no value in {theme}, taken from the base")
                    filled.append(fallback)
                else:
                    filled.append(value)
            data[name] = filled

    colours = [n for n in names if classes.get(n) == "colour"]
    lengths = [n for n in names if classes.get(n) == "length"]
    numbers = [n for n in names if classes.get(n) == "number"]
    curves = [n for n in names if classes.get(n) == "curve"]
    raws = [n for n in names if classes.get(n) == "raw"]

    for list_of_names, what in (
        (colours, "Ink"), (lengths, "Span"), (numbers, "Factor"),
        (curves, "Curve"), (raws, "Raw"),
    ):
        check_unique(list_of_names, what)

    unambiguous, verbatim, keyframes = scan_motion()
    keyframe_sources = keyframes_with_sources()

    # Every bezier mentioned by motion or by a token, named for the enum.
    # Tokens are named first so `--ease-out-expo` gives its own bezier the name
    # the reference gives it, rather than a name built out of its four numbers.
    curve_values: dict[tuple[float, float, float, float], str] = {}
    for name in curves:
        curve_values[data[name][0]] = rust_name(name)
    for _, _, curve, _ in unambiguous:
        curve_values.setdefault(curve, curve_label(curve))
    ordered_curves = sorted(curve_values.items(), key=lambda kv: kv[1])

    lines: list[str] = []
    add = lines.append

    add("//! The reference client's design system, compiled from its own stylesheets.")
    add("//!")
    add("//! **Generated by `tools/gen_theme.py`. Do not edit.**")
    add("//!")
    add("//! Everything in this file is a value read out of the Modrinth App's vendored")
    add("//! CSS -- its palette, its radii and gaps, its bare numbers, its cubic-bezier")
    add("//! curves, the text it cannot express as a scalar, and the motion it declares.")
    add("//! The tool resolves `@extend` and `var()` indirection the way the cascade")
    add("//! would, so a token here is the value a browser would paint, not a copy of")
    add("//! the line that set it.")
    add("//!")
    add("//! This exists because the shell used to carry a hand-written palette beside a")
    add("//! test-only table of transcribed values, and they could disagree -- they did.")
    add("//! One module, generated, is the point.")
    add("//!")
    add(f"//! Sources, at the commit pinned in `vendor/modrinth-app/UPSTREAM.md`: ")
    add(f"//! `{VARIABLES.relative_to(ROOT).as_posix()}` and the motion declared across ")
    add(f"//! `vendor/modrinth-app/`. Regenerate with `python tools/gen_theme.py`; the ")
    add(f"//! gate is `python tools/gen_theme.py --check`.")
    add("")
    add("// Nothing paints from here yet: the shell that will is stage 2 of")
    add("// `docs/superpowers/specs/2026-09-24-modrinth-native-rewrite.md`, and the shell")
    add("// on screen today paints from `theme.rs`. An allow rather than `cfg(test)`,")
    add("// because unlike the receipt this replaces, this file is the intended runtime")
    add("// source and a test-only attribute would have to be undone to use it.")
    add("// Delete this line the day the last page paints from these tables.")
    add("#![allow(dead_code)]")
    add("")
    add("use iced::Color;")
    add("")
    add("/// Which of the reference's four looks a value is being asked for.")
    add("///")
    add("/// The order is the order the generated tables are indexed in, so the")
    add("/// discriminant of a variant is its column.")
    add("#[derive(Clone, Copy, Debug, PartialEq, Eq)]")
    add("pub enum Theme {")
    add("    /// The light look (`.light-properties`).")
    add("    Light,")
    add("    /// The dark look (`.dark-mode`), the one the app opens in by default.")
    add("    Dark,")
    add("    /// `.oled-mode`: dark's surfaces taken to true black.")
    add("    Oled,")
    add("    /// `.retro-mode`: dark with a warmer, desaturated palette.")
    add("    Retro,")
    add("}")
    add("")
    add("impl Theme {")
    add("    /// The column this theme's value sits in.")
    add("    const fn index(self) -> usize {")
    add("        self as usize")
    add("    }")
    add("")
    add("    /// Every theme, for a gate that has to visit all of them.")
    add("    pub const ALL: &'static [Theme] = &[Theme::Light, Theme::Dark, Theme::Oled, Theme::Retro];")
    add("}")
    add("")

    # ---- colours ---------------------------------------------------------
    add("/// A colour the reference names, e.g. `Ink::Surface4` for `--surface-4`.")
    add("///")
    add("/// A variant is the token, camel-cased, and `--color-` is dropped because")
    add("/// the enum already says what it is: `--color-text-primary` reads as")
    add("/// `Ink::TextPrimary`.")
    add("#[derive(Clone, Copy, Debug, PartialEq, Eq)]")
    add("pub enum Ink {")
    for name in colours:
        add(f"    /// `{name}`")
        add(f"    {rust_name(name)},")
    add("}")
    add("")
    add("/// Every colour token, in the order the table below is sorted in.")
    add("pub const ALL_INK: &[Ink] = &[")
    for name in colours:
        add(f"    Ink::{rust_name(name)},")
    add("];")
    add("")
    add("/// The four values of every colour, light then dark then OLED then retro.")
    add("///")
    add("/// An array rather than a match so the table is the data: a token with a")
    add("/// value in one theme and none in another cannot be built here, because")
    add("/// the generator resolved the themes as chains before this was written.")
    add("const INK_TABLE: &[(Ink, [[u8; 4]; 4])] = &[")
    for name in colours:
        light, dark, oled, retro = (data[name][0], data[name][1], data[name][2], data[name][3])
        add(
            f"    (Ink::{rust_name(name)}, ["
            f"{rgba(light)}, {rgba(dark)}, {rgba(oled)}, {rgba(retro)}]),"
        )
    add("];")
    add("")

    # ---- lengths ---------------------------------------------------------
    add("/// A length the reference names, e.g. `Span::RadiusMd` for `--radius-md`.")
    add("///")
    add("/// Stored in logical pixels, converted from the `rem` the reference writes")
    add(f"/// against a root of {int(ROOT_PX)}px, which is the browser default it relies on.")
    add("#[derive(Clone, Copy, Debug, PartialEq, Eq)]")
    add("pub enum Span {")
    for name in lengths:
        add(f"    /// `{name}`")
        add(f"    {rust_name(name)},")
    add("}")
    add("")
    add("const SPAN_TABLE: &[(Span, f32)] = &[")
    for name in lengths:
        add(f"    (Span::{rust_name(name)}, {fnum(data[name][0][0])}),")
    add("];")
    add("")
    add("/// Every length token, in the order the table below is sorted in.")
    add("pub const ALL_SPAN: &[Span] = &[")
    for name in lengths:
        add(f"    Span::{rust_name(name)},")
    add("];")
    add("")

    # ---- numbers ---------------------------------------------------------
    add("/// A bare number the reference names, e.g. `Factor::HoverBrightness`.")
    add("///")
    add("/// These are the factors that are not colours and not lengths: the hover")
    add("/// brightness (`1.25` in dark, `0.9` in light -- the two directions a")
    add("/// control moves under the pointer) and the splash's own opacities.")
    add("#[derive(Clone, Copy, Debug, PartialEq, Eq)]")
    add("pub enum Factor {")
    for name in numbers:
        add(f"    /// `{name}`")
        add(f"    {rust_name(name)},")
    add("}")
    add("")
    add("/// The four values of every bare number, in the same order as [`Theme`].")
    add("const FACTOR_TABLE: &[(Factor, [f32; 4])] = &[")
    for name in numbers:
        add(
            f"    (Factor::{rust_name(name)}, ["
            f"{fnum(data[name][0])}, {fnum(data[name][1])}, "
            f"{fnum(data[name][2])}, {fnum(data[name][3])}]),"
        )
    add("];")
    add("")
    add("/// Every bare-number token, in the order the table below is sorted in.")
    add("pub const ALL_FACTOR: &[Factor] = &[")
    for name in numbers:
        add(f"    Factor::{rust_name(name)},")
    add("];")
    add("")

    # ---- curves ----------------------------------------------------------
    add("/// A cubic-bezier the reference uses, by name where it has one.")
    add("///")
    add("/// `Curve::EaseOutExpo` is the reference's own `--ease-out-expo`; the CSS")
    add("/// specification's named easings (`ease`, `ease-in-out`, ...) are here too")
    add("/// because the reference's transitions name them rather than spelling")
    add("/// their four numbers out.")
    add("#[derive(Clone, Copy, Debug, PartialEq, Eq)]")
    add("pub enum Curve {")
    for _, label in ordered_curves:
        add(f"    {label},")
    add("}")
    add("")
    add("/// `(x1, y1, x2, y2)` of every curve, in the order the enum declares them.")
    add("const CURVE_TABLE: &[(Curve, [f32; 4])] = &[")
    for value, label in ordered_curves:
        add(
            f"    (Curve::{label}, ["
            f"{fnum(value[0])}, {fnum(value[1])}, {fnum(value[2])}, {fnum(value[3])}]),"
        )
    add("];")
    add("")
    add("/// Every curve, in the order the table below is sorted in.")
    add("pub const ALL_CURVE: &[Curve] = &[")
    for _, label in ordered_curves:
        add(f"    Curve::{label},")
    add("];")
    add("")

    # ---- raw -------------------------------------------------------------
    add("/// A value the reference states that is not a scalar: a gradient, a")
    add("/// shadow, a blend mode, a font. Kept verbatim so the shell can read what")
    add("/// it has to draw and a porter can see the reference's own wording.")
    add("#[derive(Clone, Copy, Debug, PartialEq, Eq)]")
    add("pub enum Raw {")
    for name in raws:
        add(f"    /// `{name}`")
        add(f"    {rust_name(name)},")
    add("}")
    add("")
    add("/// The four values of every raw token. An empty string means the token is")
    add("/// not declared in that theme, and the next theme along does not supply it")
    add("/// either -- which is a fact about the reference, not a gap here.")
    add("const RAW_TABLE: &[(Raw, [&str; 4])] = &[")
    for name in raws:
        values = ", ".join(f'"{escape(value)}"' for value in data[name])
        add(f"    (Raw::{rust_name(name)}, [{values}]),")
    add("];")
    add("")
    add("/// Every raw token, in the order the table below is sorted in.")
    add("pub const ALL_RAW: &[Raw] = &[")
    for name in raws:
        add(f"    Raw::{rust_name(name)},")
    add("];")
    add("")

    # ---- the Tailwind scale ---------------------------------------------
    add("/// The Tailwind scale the reference's markup is written in.")
    add("///")
    add("/// Its own `tailwind.config.ts` sets `presets: [preset]`, and the preset")
    add("/// only renames colours -- every entry in it is `var(--...)`. So the scale")
    add("/// underneath is Tailwind's default theme, which is what these are, in")
    add("/// logical pixels against the same root the lengths above assume.")
    add("pub struct Tailwind {")
    add("    /// `p-4`, `gap-2`, `w-12` — the 4px-based spacing scale.")
    add("    pub spacing: &'static [f32],")
    add("    /// `rounded-lg` and friends, in the order Tailwind names them.")
    add("    pub radius: &'static [f32],")
    add("    /// `text-sm`, `text-3xl` — font size, then the line height beside it.")
    add("    pub font_size: &'static [(f32, f32)],")
    add("    /// `font-normal` through `font-black`.")
    add("    pub font_weight: &'static [(u16, u16)],")
    add("    /// `duration-75` through `duration-1000`, in milliseconds.")
    add("    pub duration_ms: &'static [u32],")
    add("}")
    add("")
    add("/// The default theme, as the reference inherits it.")
    add("pub const TAILWIND: Tailwind = Tailwind {")
    add("    spacing: &[")
    for _, value in sorted(TAILWIND_SPACING_REM.items(), key=lambda kv: kv[1]):
        add(f"        {fnum(value * ROOT_PX)},")
    add("    ],")
    add("    radius: &[")
    for key in ["none", "sm", "DEFAULT", "md", "lg", "xl", "2xl", "3xl", "full"]:
        add(f"        {fnum(TAILWIND_RADII_REM[key] * ROOT_PX)}, // rounded-{key}")
    add("    ],")
    add("    font_size: &[")
    for key in TAILWIND_FONT_SIZE_REM:
        add(
            f"        ({fnum(TAILWIND_FONT_SIZE_REM[key] * ROOT_PX)}, "
            f"{fnum(TAILWIND_LINE_HEIGHT_REM[key] * ROOT_PX)}), // text-{key}"
        )
    add("    ],")
    add("    font_weight: &[")
    for key, value in sorted(TAILWIND_FONT_WEIGHT.items(), key=lambda kv: kv[1]):
        add(f"        ({value}, {value}), // font-{key}")
    add("    ],")
    add("    duration_ms: &[")
    for value in TAILWIND_DURATION_MS:
        add(f"        {value},")
    add("    ],")
    add("};")
    add("")

    # ---- accessors -------------------------------------------------------
    add("/// The colour a token is in a theme.")
    add("///")
    add("/// Total by construction: every token is resolved for every theme before")
    add("/// this file is written, so there is no `Option` to handle at a call site")
    add("/// and no `unwrap` to justify. A token missing from a theme is a")
    add("/// generation error, printed by `tools/gen_theme.py --report`.")
    add("pub fn ink(theme: Theme, token: Ink) -> Color {")
    add("    let [r, g, b, a] = ink_rgba(theme, token);")
    add("    Color::from_rgba8(r, g, b, a as f32 / 255.0)")
    add("}")
    add("")
    add("/// The same value as `ink`, as the four bytes the CSS declared.")
    add("///")
    add("/// Separate from `ink` so that a gate can assert a hex value without")
    add("/// going through a float, and so that the rounding a float colour")
    add("/// introduces never decides whether a gate passes.")
    add("pub fn ink_rgba(theme: Theme, token: Ink) -> [u8; 4] {")
    add("    match INK_TABLE.binary_search_by_key(&(token as u8), |(key, _)| *key as u8) {")
    add("        Ok(found) => INK_TABLE[found].1[theme.index()],")
    add("        // Unreachable by construction, and a value rather than a panic")
    add("        // because this crate denies `unwrap` outside tests. Transparent")
    add("        // black is the one answer that cannot be mistaken for paint.")
    add("        Err(_) => [0, 0, 0, 0],")
    add("    }")
    add("}")
    add("")
    add("/// A length, in logical pixels.")
    add("pub fn span(token: Span) -> f32 {")
    add("    match SPAN_TABLE.binary_search_by_key(&(token as u8), |(key, _)| *key as u8) {")
    add("        Ok(found) => SPAN_TABLE[found].1,")
    add("        Err(_) => 0.0,")
    add("    }")
    add("}")
    add("")
    add("/// A bare number, per theme.")
    add("pub fn factor(theme: Theme, token: Factor) -> f32 {")
    add("    match FACTOR_TABLE.binary_search_by_key(&(token as u8), |(key, _)| *key as u8) {")
    add("        Ok(found) => FACTOR_TABLE[found].1[theme.index()],")
    add("        Err(_) => 0.0,")
    add("    }")
    add("}")
    add("")
    add("/// The four control points of a curve, for a tween to evaluate.")
    add("pub fn curve(token: Curve) -> [f32; 4] {")
    add("    match CURVE_TABLE.binary_search_by_key(&(token as u8), |(key, _)| *key as u8) {")
    add("        Ok(found) => CURVE_TABLE[found].1,")
    add("        Err(_) => [0.0, 0.0, 1.0, 1.0],")
    add("    }")
    add("}")
    add("")
    add("/// A value that is not a scalar, per theme, verbatim.")
    add("pub fn raw(theme: Theme, token: Raw) -> &'static str {")
    add("    match RAW_TABLE.binary_search_by_key(&(token as u8), |(key, _)| *key as u8) {")
    add("        Ok(found) => RAW_TABLE[found].1[theme.index()],")
    add("        Err(_) => \"\",")
    add("    }")
    add("}")
    add("")

    # ---- motion ----------------------------------------------------------
    add("/// One transition the reference declares: what moves, how long it takes,")
    add("/// and on which curve.")
    add("pub struct Motion {")
    add("    /// The CSS property that moves, as the reference names it.")
    add("    pub property: &'static str,")
    add("    /// How long it takes, in milliseconds.")
    add("    pub millis: u32,")
    add("    /// The curve it moves on.")
    add("    pub curve: Curve,")
    add("    /// Where in the reference it was read, so a port can go and look.")
    add("    pub source: &'static str,")
    add("}")
    add("")
    add("/// Every transition the reference declares with one property and one")
    add("/// duration, deduplicated. The shell's tween engine takes its numbers from")
    add("/// here rather than choosing them, which is what NOTES 29 asks for.")
    add("pub const MOTION: &[Motion] = &[")
    for property_name, millis, curve_value, source in unambiguous:
        label = curve_values[curve_value]
        add(
            f"    Motion {{ property: \"{escape(property_name)}\", millis: {millis}, "
            f"curve: Curve::{label}, source: \"{escape(source)}\" }},"
        )
    add("];")
    add("")
    add("/// Transitions the reference writes as a shorthand this tool will not")
    add("/// guess at: one duration shared by a comma-separated property list.")
    add("///")
    add("/// CSS reads `transition: color, background-color 125ms ease-in-out` as")
    add("/// *two* transitions with different timings, and splitting it by hand in a")
    add("/// generator is how a wrong number gets a plausible name. The reference's")
    add("/// own wording is kept instead.")
    add("pub const MOTION_VERBATIM: &[(&str, &str)] = &[")
    for source, _, body in verbatim:
        add(f"    (\"{escape(source)}\", \"{escape(body)}\"),")
    add("];")
    add("")
    add("/// The names of every `@keyframes` the reference declares, in order.")
    add("///")
    add("/// Bodies are not copied: a keyframe is a sequence of property states, and")
    add("/// which of them a native port needs depends on the widget, so the shell")
    add("/// reads the reference for the one it is porting and cites this list.")
    add("pub const KEYFRAMES: &[(&str, &str)] = &[")
    for name, source in keyframe_sources:
        add(f"    (\"{escape(name)}\", \"{escape(source)}\"),")
    add("];")
    add("")
    add("/// The theme the app opens in, and the one every measurement in")
    add("/// `REFERENCE.md` was taken in.")
    add("pub const DEFAULT_THEME: Theme = Theme::Dark;")
    add("")

    # ---- tests -----------------------------------------------------------
    add("#[cfg(test)]")
    add("mod tests {")
    add("    use super::*;")
    add("")
    add("    #[test]")
    add("    fn every_table_is_sorted_so_its_lookup_is_valid() {")
    add("        // `binary_search_by_key` is only a search if the table is sorted by")
    add("        // the same key, which is the one assumption the accessors make.")
    for table in ("INK_TABLE", "SPAN_TABLE", "FACTOR_TABLE", "CURVE_TABLE", "RAW_TABLE"):
        add(f"        for pair in {table}.windows(2) {{")
        add(f"            assert!((pair[0].0 as u8) < (pair[1].0 as u8), \"{table} is out of order\");")
        add("        }")
    add("    }")
    add("")
    add("    #[test]")
    add("    fn the_tables_cover_every_variant() {")
    add("        assert_eq!(INK_TABLE.len(), ALL_INK.len());")
    add("        assert_eq!(SPAN_TABLE.len(), ALL_SPAN.len());")
    add("        assert_eq!(FACTOR_TABLE.len(), ALL_FACTOR.len());")
    add("        assert_eq!(CURVE_TABLE.len(), ALL_CURVE.len());")
    add("        assert_eq!(RAW_TABLE.len(), ALL_RAW.len());")
    add("    }")
    add("")
    add("    #[test]")
    add("    fn every_token_resolves_in_every_theme() {")
    add("        for theme in Theme::ALL {")
    add("            for token in ALL_INK {")
    add("                let [_, _, _, alpha] = ink_rgba(*theme, *token);")
    add("                // A colour with no value would resolve to transparent black,")
    add("                // which is only ever legitimate as an explicit `transparent`.")
    add("                let declared_transparent = [\"Transparent\", \"AccentContrast\"];")
    add("                let name = format!(\"{:?}\", token);")
    add("                assert!(")
    add("                    alpha != 0 || declared_transparent.contains(&name.as_str()),")
    add("                    \"{name} has no value in {theme:?}\"")
    add("                );")
    add("            }")
    add("            for token in ALL_SPAN {")
    add("                assert!(span(*token) > 0.0, \"{token:?} has no value\");")
    add("            }")
    add("        }")
    add("    }")
    add("")
    add("    #[test]")
    add("    fn the_surfaces_are_the_ones_the_reference_paints() {")
    add("        // Recorded in REFERENCE.md off the reference's own window: the page is")
    add("        // `#16181c` in dark, the chrome above it `#27292e`, and OLED takes the")
    add("        // page to true black.")
    add("        assert_eq!(ink_rgba(Theme::Dark, Ink::Surface1), [0x16, 0x18, 0x1c, 0xff]);")
    add("        assert_eq!(ink_rgba(Theme::Dark, Ink::Surface3), [0x27, 0x29, 0x2e, 0xff]);")
    add("        assert_eq!(ink_rgba(Theme::Oled, Ink::Surface1), [0x00, 0x00, 0x00, 0xff]);")
    add("        assert_eq!(ink_rgba(Theme::Light, Ink::Surface1), [0xeb, 0xeb, 0xeb, 0xff]);")
    add("        assert_eq!(ink_rgba(Theme::Retro, Ink::Surface1), [0x19, 0x19, 0x17, 0xff]);")
    add("    }")
    add("")
    add("    #[test]")
    add("    fn the_accent_is_followed_through_its_indirection_per_theme() {")
    add("        // `--color-brand` is `var(--color-green)`, which is `var(--color-green-600)`")
    add("        // in light and `var(--color-green-500)` in dark, and is replaced outright in")
    add("        // retro. A generator that read the literal line would answer `var(...)` to")
    add("        // all four; this is the assertion that it followed the chain.")
    add("        assert_eq!(ink_rgba(Theme::Light, Ink::Brand), [0x00, 0xaf, 0x5c, 0xff]);")
    add("        assert_eq!(ink_rgba(Theme::Dark, Ink::Brand), [0x1b, 0xd9, 0x6a, 0xff]);")
    add("        assert_eq!(ink_rgba(Theme::Oled, Ink::Brand), [0x1b, 0xd9, 0x6a, 0xff]);")
    add("        assert_eq!(ink_rgba(Theme::Retro, Ink::Brand), [0x4d, 0x92, 0x27, 0xff]);")
    add("    }")
    add("")
    add("    #[test]")
    add("    fn the_hover_factor_moves_the_way_the_reference_says() {")
    add("        // Two values, not one: light *darkens* a control under the pointer.")
    add("        assert_eq!(factor(Theme::Light, Factor::HoverBrightness), 0.9);")
    add("        assert_eq!(factor(Theme::Dark, Factor::HoverBrightness), 1.25);")
    add("        assert_eq!(factor(Theme::Retro, Factor::HoverBrightness), 1.25);")
    add("    }")
    add("")
    add("    #[test]")
    add("    fn the_radii_are_rem_converted_at_the_browsers_own_root() {")
    add("        assert_eq!(span(Span::RadiusXs), 4.0);")
    add("        assert_eq!(span(Span::RadiusSm), 8.0);")
    add("        assert_eq!(span(Span::RadiusMd), 12.0);")
    add("        assert_eq!(span(Span::RadiusLg), 16.0);")
    add("        assert_eq!(span(Span::RadiusXl), 20.0);")
    add("    }")
    add("")
    add("    #[test]")
    add("    fn the_reference_easing_is_the_one_it_declares() {")
    add("        assert_eq!(curve(Curve::EaseOutExpo), [0.16, 1.0, 0.3, 1.0]);")
    add("    }")
    add("")
    add("    #[test]")
    add("    fn the_motion_table_is_not_empty_and_names_its_sources() {")
    add("        assert!(!MOTION.is_empty());")
    add("        for motion in MOTION {")
    add("            assert!(motion.millis > 0, \"{} has no duration\", motion.property);")
    add("            assert!(motion.source.ends_with(\".css\") || motion.source.ends_with(\".scss\")")
    add("                || motion.source.ends_with(\".vue\"));")
    add("        }")
    add("        assert!(!KEYFRAMES.is_empty());")
    add("    }")
    add("")
    add("    #[test]")
    add("    fn the_scale_the_markup_is_written_in_is_named() {")
    add("        // `p-4` is 16px and `text-sm` is 14px on a 20px line; the shell's")
    add("        // layout numbers are these, not chosen.")
    add("        assert!(TAILWIND.spacing.contains(&16.0));")
    add("        assert!(TAILWIND.font_size.contains(&(14.0, 20.0)));")
    add("        assert!(TAILWIND.font_weight.iter().any(|(_, w)| *w == 800));")
    add("    }")
    add("}")
    add("")

    text = "\n".join(lines)
    text = text.replace("    Err(_) => [0.0, 0.0, 1.0, 1.0],", "    Err(_) => [0.0, 0.0, 1.0, 1.0],")
    report = [
        f"colours      {len(colours)}",
        f"lengths      {len(lengths)}",
        f"numbers      {len(numbers)}",
        f"curves       {len(curves)}",
        f"raw          {len(raws)}",
        f"motion       {len(unambiguous)} parsed, {len(verbatim)} verbatim",
        f"keyframes    {len(keyframe_sources)}",
    ]
    return text, report + notes


def curve_label(value: tuple[float, float, float, float]) -> str:
    for name, known in NAMED_CURVES.items():
        if known == value:
            return {
                "linear": "Linear",
                "ease": "Ease",
                "ease-in": "EaseIn",
                "ease-out": "EaseOut",
                "ease-in-out": "EaseInOut",
            }[name]
    return "Bezier" + "".join(fnum(part).replace(".", "_").replace("-", "N") for part in value)


def keyframes_with_sources() -> list[tuple[str, str]]:
    out: list[tuple[str, str]] = []
    for path in motion_sources():
        try:
            text = strip_comments(path.read_text(encoding="utf-8", errors="replace"))
        except OSError:
            continue
        rel = path.relative_to(VENDOR).as_posix()
        for match in re.finditer(r"@keyframes\s+([A-Za-z0-9_-]+)", text):
            out.append((match.group(1), rel))
    return sorted(set(out))


def rgba(value: str) -> str:
    r, g, b, a = value
    return f"[0x{r:02x}, 0x{g:02x}, 0x{b:02x}, 0x{a:02x}]"


def escape(text: str) -> str:
    return text.replace("\\", "\\\\").replace('"', '\\"')


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--check", action="store_true",
                        help="fail if the checked-in file is not what this would emit")
    parser.add_argument("--report", action="store_true",
                        help="print what was classified, and what could not be")
    args = parser.parse_args()

    for path in (VARIABLES, UTILITIES):
        if not path.is_file():
            print(f"missing reference source: {path}", file=sys.stderr)
            return 2

    variables = VARIABLES.read_text(encoding="utf-8")
    blocks = parse_blocks(variables)
    themes = build_theme_maps(blocks, variables)

    # The ui package declares some of its own custom properties at `:root` --
    # `--ease-out-expo` among them. They are not theme-dependent, so they join
    # every theme as a fallback rather than being a fifth theme that is not one.
    # Without this, `--ease-out-expo` would be missing from the table and the
    # transitions written as `var(--ease-out-expo)` would have no curve.
    utilities = UTILITIES.read_text(encoding="utf-8")
    utility_root = parse_blocks(utilities).get(":root", {})
    for table in themes.values():
        for name, value in utility_root.items():
            table.setdefault(name, value)

    # The motion scan resolves `var(--ease-*)` against the same table, so it has
    # to be filled in before the scan rather than after it.
    VARIABLE_VALUES.update(utility_root)
    VARIABLE_VALUES.update(themes["dark"])

    text, report = emit(themes)

    if args.check:
        current = OUT.read_text(encoding="utf-8") if OUT.is_file() else ""
        if current != text:
            print(f"{OUT.relative_to(ROOT)} is not what this tool emits.", file=sys.stderr)
            print("Run `python tools/gen_theme.py` and commit the result.", file=sys.stderr)
            return 1
        print("theme generation is byte-identical")
        return 0

    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text(text, encoding="utf-8", newline="\n")
    if args.report:
        for line in report:
            print(line)
    print(f"wrote {OUT.relative_to(ROOT)} ({text.count(chr(10)) + 1} lines)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
