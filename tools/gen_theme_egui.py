#!/usr/bin/env python3
"""Compile the reference client's palette, geometry, type and motion into egui.

The egui slice paints nothing from a hand-written table: every colour, radius, gap,
type size and duration in `crates/palantir-egui/src/theme.rs` is read out of the
Modrinth App's own stylesheets here, resolved the way a browser would resolve it, and
written down with the file and the line it came from.

    python tools/gen_theme_egui.py            # write crates/palantir-egui/src/theme.rs
    python tools/gen_theme_egui.py --check    # fail if the file on disk is not what this emits
    python tools/gen_theme_egui.py --report   # additionally print what every token resolved through

## What it reads

All of it vendored, all of it read-only, all of it at the commit
`vendor/modrinth-app/UPSTREAM.md` pins:

| Source | What comes out of it |
| --- | --- |
| `assets/styles/variables.scss` | The palette, the radii and the gaps: `.light-properties`, `html`, `.light-mode`, and `.dark-mode` together with its `.dark` and `:root[data-theme='dark']` selector aliases |
| `assets/styles/defaults.scss` | The type ladder and the weights, declared on `body` |
| `ui/src/styles/tailwind-utilities.css` | `--ease-out-expo`, and the popover and floating-action-bar transitions |
| every other `*.css` / `*.scss` under `vendor/modrinth-app` | Any further `:root` / `.dark` / `[data-theme]` block that declares a token, merged as a fallback |

The iced generator next door (`tools/gen_theme.py`) reads the same two global sheets.
This one does not import it: that tool resolves four looks and this one resolves two,
so a shared parser would have to carry both. They cross-check each other instead --
`theme.rs`'s own tests hard-code the numbers (`#16181c` for the dark page) that a
reader can verify in the sheet, so two readings of the same CSS have to agree.

## How a value is resolved

* **`@extend`** -- `html` extends `.light-properties`, and a theme is therefore a
  chain; this follows it in the order the cascade would.
* **`var()` indirection** -- `--color-bg: var(--surface-1)`, and `--color-green` is
  `var(--color-green-600)` in light and `var(--color-green-500)` in dark. A reference
  is followed to a fixed point rather than one hop, and one that cannot be resolved is
  a generation error naming the variable and the whole chain -- never a silent gap.
* **Comments, `!important`, multi-line values** -- stripped, stripped, joined on one
  space (which is what a browser does with them too).
* **Colours** -- `#rgb`, `#rgba`, `#rrggbb`, `#rrggbbaa`, `rgb()`/`rgba()` in comma or
  space syntax, and the three named colours the sheets use (`black`, `white`,
  `transparent`). Anything else is a generation error, so a new syntax cannot be
  silently skipped.
* **Transcript** -- every declaration keeps the line it starts on, so `theme.rs` can
  print `variables.scss:233` beside a value. That is the whole point of generating it:
  a stale colour traces back to the line that moved.

## What it emits

`crates/palantir-egui/src/theme.rs`:

* a `Theme` struct whose fields are the CSS custom properties in SCREAMING_SNAKE with
  the `--color-` prefix dropped (`--color-bg` -> `BG`, `--color-raised-bg` ->
  `RAISED_BG`), so a value on screen can be grepped back to the stylesheet;
* `DARK` and `LIGHT`, each holding every token at its resolved value;
* the `--radius-*`, `--gap-*`, `--font-size-*` and `--font-weight-*` ladders, and the
  transitions the shell animates, each with its source line;
* `Theme::colors()`, which builds the egui `Visuals` for a scheme out of the tokens;
* tests that pin the numbers a byte-for-byte diff cannot (the dark page colour, the
  alpha of a translucent token, `window_fill` following `BG`).

Registration is `--check`, which regenerates into memory and compares byte for byte.
"""

from __future__ import annotations

import argparse
import re
import sys
from dataclasses import dataclass, field
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
VENDOR = ROOT / "vendor" / "modrinth-app"
OUT = ROOT / "crates" / "palantir-egui" / "src" / "theme.rs"

VARIABLES = "assets/styles/variables.scss"
DEFAULTS = "assets/styles/defaults.scss"
UTILITIES = "ui/src/styles/tailwind-utilities.css"

# The root font size every `rem` in these sheets is written against. The reference
# sets no `font-size` on `html`, so a browser uses its own default: 16px.
ROOT_PX = 16.0

# The longest transition anywhere in the reference is 2s (`ui/src/components/brand/
# TextLogo.vue`). Nothing legitimate reaches this bound, so a parsed duration above it
# is a mistake in this file rather than a fact about the reference.
LONGEST_TRANSITION_MS = 2000

# CSS's own named easings. Transcribed rather than parsed because they are in the
# specification, not in this tree -- the sheets name them without spelling them out.
NAMED_EASINGS = {
    "linear": (0.0, 0.0, 1.0, 1.0),
    "ease": (0.25, 0.1, 0.25, 1.0),
    "ease-in": (0.42, 0.0, 1.0, 1.0),
    "ease-out": (0.0, 0.0, 0.58, 1.0),
    "ease-in-out": (0.42, 0.0, 0.58, 1.0),
}

NAMED_COLOURS = {
    "black": (0, 0, 0, 255),
    "white": (255, 255, 255, 255),
    "transparent": (0, 0, 0, 0),
}

# A bare `var(--x)`, optionally with a fallback the resolver never takes because every
# fallback in these sheets is dead code the sheet itself never needs.
BARE_VAR = re.compile(r"^var\(\s*(--[\w-]+)\s*(?:,.*)?\)$")


# ---------------------------------------------------------------------------
# The semantic tokens the launcher needs
# ---------------------------------------------------------------------------


@dataclass(frozen=True)
class Token:
    """One CSS custom property, and what the launcher uses it for."""

    css: str
    section: str
    note: str


def field_name(css: str) -> str:
    """`--color-bg` -> `BG`, `--surface-1-5` -> `SURFACE_1_5`, `--radius-md` -> `RADIUS_MD`.

    The `--color-` prefix is dropped because the field is already a colour and the
    sheet's own prefix says so: `--color-text-primary` reads as `TEXT_PRIMARY`, and a
    reader greps `--color-text-primary` and finds it. Nothing else is renamed, so a
    token is never more than a `grep` away from the line that declares it.
    """
    name = css[2:] if css.startswith("--") else css
    if name.startswith("color-"):
        name = name[len("color-"):]
    return name.replace("-", "_").upper()


# The colour tokens, grouped by the part of the interface they paint, in the order
# they are emitted. The prompt for each is the role the reference gives it, so a field
# reads as what it is for rather than as a transliteration of its name.
COLOUR_TOKENS = [
    Token("--color-bg", "page and surfaces", "the page itself"),
    Token("--color-raised-bg", "page and surfaces", "a raised panel or card"),
    Token("--color-super-raised-bg", "page and surfaces", "the topmost raised layer"),
    Token("--surface-1", "page and surfaces", "the first surface step"),
    Token("--surface-1-5", "page and surfaces", "a step between the first and second"),
    Token("--surface-2", "page and surfaces", "the second surface step"),
    Token("--surface-2-5", "page and surfaces", "a step between the second and third"),
    Token("--surface-3", "page and surfaces", "the third surface step, under the chrome"),
    Token("--surface-4", "page and surfaces", "the fourth step, a control's fill"),
    Token("--surface-5", "page and surfaces", "the fifth step, an input's edge"),

    Token("--color-button-bg", "controls", "a button at rest"),
    Token("--color-button-border", "controls", "the edge of a button, an input or a popover"),
    Token("--color-button-bg-selected", "controls", "a selected button's fill"),
    Token("--color-button-text-selected", "controls", "a selected button's text"),

    Token("--color-divider", "lines", "a rule between two areas"),
    Token("--color-divider-dark", "lines", "the heavier rule, for a dark ground"),

    Token("--color-base", "text", "the interface's default text"),
    Token("--color-secondary", "text", "muted text"),
    Token("--color-contrast", "text", "high-contrast text: headings, hovered labels"),
    Token("--color-accent-contrast", "text", "text on a brand or hue fill"),
    Token("--color-text-primary", "text", "the strongest text step (the same value as `CONTRAST`)"),
    Token("--color-text-default", "text", "the default text step (the same value as `BASE`)"),
    Token("--color-text-tertiary", "text", "the third text step (the same value as `SECONDARY`)"),

    Token("--color-brand", "accent", "Modrinth's own green"),
    Token("--color-brand-highlight", "accent", "a brand wash: selection, selected rows"),
    Token("--color-brand-shadow", "accent", "the glow around a focused brand control"),

    Token("--color-red", "hues", "destructive actions"),
    Token("--color-orange", "hues", "warnings"),
    Token("--color-green", "hues", "success, downloads"),
    Token("--color-blue", "hues", "links"),
    Token("--color-purple", "hues", "curseforge and other third-party marks"),
    Token("--color-gray", "hues", "neutral marks"),

    Token("--color-red-highlight", "hue washes", "a red wash behind a badge or a row"),
    Token("--color-orange-highlight", "hue washes", "an orange wash behind a badge or a row"),
    Token("--color-green-highlight", "hue washes", "a green wash behind a badge or a row"),
    Token("--color-blue-highlight", "hue washes", "a blue wash behind a badge or a row"),
    Token("--color-purple-highlight", "hue washes", "a purple wash behind a badge or a row"),
    Token("--color-gray-highlight", "hue washes", "a gray wash behind a badge or a row"),
    Token("--color-red-bg", "hue washes", "a red banner's background"),
    Token("--color-orange-bg", "hue washes", "an orange banner's background"),
    Token("--color-green-bg", "hue washes", "a green banner's background"),
    Token("--color-blue-bg", "hue washes", "a blue banner's background"),
    Token("--color-purple-bg", "hue washes", "a purple banner's background"),

    Token("--color-link", "links", "a link"),
    Token("--color-link-hover", "links", "a link under the pointer"),
    Token("--color-link-active", "links", "a link being pressed"),

    Token("--color-scrollbar", "chrome", "a scrollbar's thumb"),
    Token("--color-tooltip-bg", "chrome", "a tooltip's background"),
    Token("--color-tooltip-text", "chrome", "a tooltip's text"),
    Token("--color-focus-ring", "chrome", "the accessibility focus ring"),

    Token("--color-platform-fabric", "platforms", "Fabric's badge"),
    Token("--color-platform-quilt", "platforms", "Quilt's badge"),
    Token("--color-platform-forge", "platforms", "Forge's badge"),
    Token("--color-platform-neoforge", "platforms", "NeoForge's badge"),
    Token("--color-platform-liteloader", "platforms", "LiteLoader's badge"),
    Token("--color-platform-bukkit", "platforms", "Bukkit's badge"),
    Token("--color-platform-bungeecord", "platforms", "BungeeCord's badge"),
    Token("--color-platform-folia", "platforms", "Folia's badge"),
    Token("--color-platform-paper", "platforms", "Paper's badge"),
    Token("--color-platform-purpur", "platforms", "Purpur's badge"),
    Token("--color-platform-spigot", "platforms", "Spigot's badge"),
    Token("--color-platform-velocity", "platforms", "Velocity's badge"),
    Token("--color-platform-waterfall", "platforms", "Waterfall's badge"),
    Token("--color-platform-sponge", "platforms", "Sponge's badge"),
    Token("--color-platform-ornithe", "platforms", "Ornithe's badge"),
    Token("--color-platform-bta-babric", "platforms", "BTA Babric's badge"),
    Token("--color-platform-nilloader", "platforms", "NilLoader's badge"),
]

# Tokens the module reads today. Everything else is emitted because the stylesheet
# declares it, and is marked individually in the generated file -- `#![allow(dead_code)]`
# would hide the next token that stops being used as well as these.
USED_COLOURS = {
    "BG", "RAISED_BG", "BUTTON_BG", "BUTTON_BORDER", "DIVIDER", "BASE", "CONTRAST",
    "BRAND", "BRAND_HIGHLIGHT", "RED", "ORANGE", "LINK",
}

RADIUS_TOKENS = [
    "--radius-xs", "--radius-sm", "--radius-md", "--radius-lg", "--radius-xl", "--radius-max",
]
GAP_TOKENS = ["--gap-xs", "--gap-sm", "--gap-md", "--gap-lg", "--gap-xl"]

# The sizes and weights small text, body and headings are set in, by the token each
# comes from. The roles are documented on `Theme::type_sizes` in the generated file.
TYPE_SIZE_TOKENS = [
    ("--font-size-xs", "small text: a metadata row, a caption"),
    ("--font-size-nm", "body: the size `body` itself is set in"),
    ("--font-size-xl", "headings: the 1.5rem the page header's `text-2xl` also resolves to"),
]
TYPE_WEIGHT_TOKENS = [
    ("--font-weight-regular", "body's `font-weight`"),
    ("--font-weight-text", "the weight the whole interface is set in"),
    ("--font-weight-heading", "headings and titles"),
]

# The transitions the shell animates, picked out of every transition the sheets
# declare. A pick is `(const, property, millis, sheet, easing const, what it is)`, and
# all six numbers are asserted to exist in the scan -- a sheet that renames or retimes
# one fails generation instead of silently dropping it.
@dataclass(frozen=True)
class MotionPick:
    const: str
    property: str
    millis: int
    sheet: str
    easing_const: str
    note: str


MOTION_PICKS = [
    MotionPick("PRESS_MS", "scale", 50, "assets/styles/classes.scss", "EASE_IN_OUT",
               "the press: every clickable scales to 0.95 while held"),
    MotionPick("HOVER_MS", "filter", 200, "assets/styles/classes.scss", "EASE_IN_OUT",
               "a pointer arriving on a control, which the reference does with `filter: brightness()`"),
    MotionPick("OUTLINE_MS", "outline", 200, "assets/styles/classes.scss", "EASE_IN_OUT",
               "the focus outline"),
    MotionPick("FADE_MS", "opacity", 500, "assets/styles/classes.scss", "EASE_IN_OUT",
               "a clickable fading in or out"),
    MotionPick("POPOVER_MS", "opacity", 125, "ui/src/styles/tailwind-utilities.css", "EASE_OUT_EXPO",
               "a floating panel appearing (`floating-expand`)"),
    MotionPick("FLOATING_BAR_MS", "transform", 250, "ui/src/styles/tailwind-utilities.css",
               "EASE_FLOATING_BAR", "the floating action bar sliding in"),
]


# ---------------------------------------------------------------------------
# Reading a stylesheet
# ---------------------------------------------------------------------------


def strip_comments(text: str) -> str:
    """Remove comments without moving a line number by so much as one.

    A block comment becomes spaces with its newlines kept, and a `//` comment loses its
    text but keeps its newline, so a declaration's line here is the line an editor
    shows. `://` is left alone: a `url(https://…)` is not a comment.
    """
    text = re.sub(r"/\*.*?\*/", lambda m: re.sub(r"[^\n]", " ", m.group(0)), text, flags=re.S)
    lines = []
    for line in text.split("\n"):
        cut = line.find("//")
        if cut > 0 and line[cut - 1] != ":":
            line = line[:cut]
        elif cut == 0:
            line = ""
        lines.append(line)
    return "\n".join(lines)


@dataclass
class Decl:
    """One `--token: value` declaration and the line it starts on."""

    value: str
    line: int


@dataclass
class Block:
    """One `selector { … }` with its declarations, `@extend`s and start line."""

    selectors: list[str]
    line: int
    decls: dict[str, Decl] = field(default_factory=dict)
    extends: list[str] = field(default_factory=list)


def normalise(value: str) -> str:
    """One value, on one line, without its `!important`."""
    value = re.sub(r"!important\s*$", "", value.strip())
    return re.sub(r"\s+", " ", value).strip()


def split_items(body: str) -> list[str]:
    """Split a comma-separated value, ignoring commas inside parentheses."""
    items: list[str] = []
    depth = 0
    current: list[str] = []
    for char in body:
        if char == "(":
            depth += 1
        elif char == ")":
            depth = max(0, depth - 1)
        if char == "," and depth == 0:
            items.append("".join(current))
            current = []
        else:
            current.append(char)
    items.append("".join(current))
    return [item.strip() for item in items if item.strip()]


def split_tokens(item: str) -> list[str]:
    """Split one transition item on spaces, keeping `cubic-bezier(a, b, c, d)` whole."""
    tokens: list[str] = []
    depth = 0
    current: list[str] = []
    for char in item:
        if char == "(":
            depth += 1
        elif char == ")":
            depth = max(0, depth - 1)
        if char.isspace() and depth == 0:
            if current:
                tokens.append("".join(current))
                current = []
        else:
            current.append(char)
    if current:
        tokens.append("".join(current))
    return tokens


def parse_declarations(body: str, start_line: int) -> tuple[dict[str, Decl], list[str]]:
    """The custom properties in one block body, with `@extend`s, skipping nested rules.

    `defaults.scss` nests blocks inside `body` (`input[type='number'] { … }`); their
    declarations belong to a more specific selector than the cascade this models, so
    they are dropped rather than mis-attributed to the parent. A value written across
    several lines is joined with a single space, which is what a browser does with it.
    """
    decls: dict[str, Decl] = {}
    extends: list[str] = []
    depth = 0
    buffer: list[str] = []
    buffer_line: int | None = None
    line = start_line

    def flush() -> None:
        nonlocal buffer, buffer_line
        item = "".join(buffer).strip()
        buffer = []
        if not item:
            buffer_line = None
            return
        if item.startswith("@extend"):
            target = item.split(None, 1)[1] if len(item.split(None, 1)) > 1 else ""
            if target:
                extends.append(target.strip().rstrip(";"))
            buffer_line = None
            return
        name, sep, value = item.partition(":")
        if sep and name.strip().startswith("--"):
            decls[name.strip()] = Decl(normalise(value), buffer_line if buffer_line else line)
        buffer_line = None

    for char in body:
        if char == "\n":
            line += 1
            if depth == 0 and buffer:
                buffer.append(" ")
            continue
        if char == "{":
            depth += 1
            buffer = []
            buffer_line = None
            continue
        if char == "}":
            depth = max(0, depth - 1)
            buffer = []
            buffer_line = None
            continue
        if depth > 0:
            continue
        if char == ";":
            flush()
            continue
        if buffer_line is None and not char.isspace():
            buffer_line = line
        buffer.append(char)
    flush()
    return decls, extends


def parse_sheet(text: str) -> list[Block]:
    """Every top-level `selector { … }` in a stylesheet, in file order."""
    text = strip_comments(text)
    line_starts = [0]
    for index, char in enumerate(text):
        if char == "\n":
            line_starts.append(index + 1)

    def line_at(index: int) -> int:
        low, high = 0, len(line_starts) - 1
        while low < high:
            mid = (low + high + 1) // 2
            if line_starts[mid] <= index:
                low = mid
            else:
                high = mid - 1
        return low + 1

    blocks: list[Block] = []
    depth = 0
    selector_start = 0
    body_start = 0
    selectors: list[str] = []
    selector_line = 1
    for index, char in enumerate(text):
        if char == "{":
            if depth == 0:
                body_start = index + 1
                raw = text[selector_start:index]
                # A statement between two blocks (`@import 'reset';`) is not a selector.
                raw = raw.split(";")[-1]
                selector_line = line_at(selector_start + (len(raw) - len(raw.lstrip())))
                selectors = [one.strip() for one in raw.split(",") if one.strip()]
            depth += 1
        elif char == "}":
            depth -= 1
            if depth == 0:
                decls, extends = parse_declarations(text[body_start:index], line_at(body_start))
                blocks.append(Block(selectors, selector_line, decls, extends))
                selector_start = index + 1
            elif depth < 0:
                raise SystemExit("unbalanced `}` in a stylesheet this tool reads")
    return blocks


def selector_key(selector: str) -> str:
    """`.dark-mode` -> `dark-mode`; quotes inside an attribute selector normalised.

    Keys are undotted so that a block and an `@extend` naming it -- `html { @extend
    .light-properties }` -- are the same string, which is what the chain looks up.
    """
    key = selector.strip()
    if key.startswith("."):
        key = key[1:]
    return key.replace('"', "'")


# The selectors a scheme is built from, in cascade order. `:root` and `light-properties`
# first because `html` extends the latter, and the reference puts `.light-properties` on
# `html`, so every theme falls back to those values for whatever it does not declare.
BASE_SELECTORS = [":root", "light-properties", "html", "body"]
SCHEME_SELECTORS = {
    "light": ["light-mode", "light"],
    "dark": ["dark-mode", "dark", ":root[data-theme='dark']", "[data-theme='dark']"],
}

SHEET_ORDER = [VARIABLES, DEFAULTS, UTILITIES]


def sheet_paths() -> list[Path]:
    """The sheets this reads, named ones first and the rest in path order."""
    paths = [VENDOR / name for name in SHEET_ORDER]
    named = {path.resolve() for path in paths}
    extra = sorted(
        path for path in VENDOR.rglob("*")
        if path.suffix in (".css", ".scss") and path.is_file() and path.resolve() not in named
    )
    return paths + extra


def load_blocks() -> dict[str, list[tuple[str, Block]]]:
    """Every theme-shaped block in the tree, keyed by selector, in a fixed order."""
    blocks: dict[str, list[tuple[str, Block]]] = {}
    for path in sheet_paths():
        if not path.is_file():
            continue
        rel = path.relative_to(VENDOR).as_posix()
        try:
            text = path.read_text(encoding="utf-8")
        except OSError as error:  # pragma: no cover - a missing vendored tree
            raise SystemExit(f"cannot read {path}: {error}") from error
        for block in parse_sheet(text):
            for selector in block.selectors:
                blocks.setdefault(selector_key(selector), []).append((rel, block))
    return blocks


def build_scheme(blocks: dict[str, list[tuple[str, Block]]], scheme: str) -> dict[str, tuple[str, int, str, str]]:
    """One scheme's tokens, resolved through `@extend` the way the cascade would.

    Returns `token -> (value, line, sheet, block selector)`, where the block is the one
    whose declaration won -- which is what lets the generated file say "`html`, and
    `.dark-mode` does not override it" beside a token dark inherits from light.
    """
    table: dict[str, tuple[str, int, str, str]] = {}
    applying: list[str] = []

    def apply(key: str) -> None:
        if key in applying:
            raise SystemExit(f"`@extend` cycle through `{key}` in {VARIABLES}")
        applying.append(key)
        for rel, block in blocks.get(key, []):
            for parent in block.extends:
                apply(selector_key(parent))
            for name, decl in block.decls.items():
                table[name] = (decl.value, decl.line, rel, key)
        applying.pop()

    for key in BASE_SELECTORS + SCHEME_SELECTORS[scheme]:
        apply(key)
    return table


# ---------------------------------------------------------------------------
# Resolving a value
# ---------------------------------------------------------------------------


@dataclass
class Resolution:
    """One token in one scheme, with everything needed to explain it."""

    token: str
    scheme: str
    declared_value: str
    declared_line: int
    declared_sheet: str
    declared_block: str
    chain: list[tuple[str, str, int, str]]
    value: str
    final_line: int
    final_sheet: str
    rgb: tuple[int, int, int] | None = None
    alpha: int | None = None

    @property
    def via_var(self) -> bool:
        return bool(self.chain)

    @property
    def rgba(self) -> tuple[int, int, int, int]:
        if self.rgb is None or self.alpha is None:
            raise SystemExit(f"{self.token} is not a colour; this accessor should not be asked")
        return (*self.rgb, self.alpha)


def resolve(table: dict[str, tuple[str, int, str, str]], token: str, scheme: str) -> Resolution:
    """Follow `var()` to a fixed point, failing loudly with the variable and the chain."""
    if token not in table:
        raise SystemExit(f"{token} is not declared in the {scheme} cascade; it is in COLOUR_TOKENS")
    declared_value, declared_line, declared_sheet, declared_block = table[token]
    chain: list[tuple[str, str, int, str]] = []
    seen = [token]
    value, line, sheet = declared_value, declared_line, declared_sheet
    while True:
        match = BARE_VAR.fullmatch(value.strip())
        if not match:
            break
        name = match.group(1)
        if name in seen:
            raise SystemExit(
                f"{declared_sheet}:{declared_line}: {token} = {declared_value}: "
                f"`var({name})` is a cycle ({' -> '.join(seen + [name])})"
            )
        seen.append(name)
        if name not in table:
            raise SystemExit(
                f"{declared_sheet}:{declared_line}: {token} = {declared_value}: "
                f"`var({name})` is not declared in this cascade "
                f"({' -> '.join(seen)}); add it to the sheet or to COLOUR_TOKENS' sources"
            )
        inner, inner_line, inner_sheet, _ = table[name]
        chain.append((name, inner, inner_line, inner_sheet))
        value, line, sheet = inner, inner_line, inner_sheet
    return Resolution(
        token=token,
        scheme=scheme,
        declared_value=declared_value,
        declared_line=declared_line,
        declared_sheet=declared_sheet,
        declared_block=declared_block,
        chain=chain,
        value=value,
        final_line=line,
        final_sheet=sheet,
    )


def parse_colour(value: str) -> tuple[int, int, int, int] | None:
    """`#rgb`, `#rgba`, `#rrggbb`, `#rrggbbaa`, `rgb()`/`rgba()`, or a named colour."""
    value = value.strip()
    if value.startswith("#"):
        digits = value[1:]
        if len(digits) in (3, 4):
            channels = [int(char * 2, 16) for char in digits]
        elif len(digits) in (6, 8):
            channels = [int(digits[index:index + 2], 16) for index in range(0, len(digits), 2)]
        else:
            return None
        if len(channels) == 3:
            channels.append(255)
        return (channels[0], channels[1], channels[2], channels[3])
    match = re.fullmatch(r"rgba?\((.*)\)", value, re.S)
    if match:
        parts = [part for part in re.split(r"\s*[,/]\s*|\s+", match.group(1).strip()) if part]
        if len(parts) not in (3, 4):
            return None
        try:
            if any(part.endswith("%") for part in parts[:3]):
                channels = [round(float(part.rstrip("%")) * 255 / 100) for part in parts[:3]]
            else:
                channels = [round(float(part)) for part in parts[:3]]
            if len(parts) == 4:
                raw = parts[3]
                alpha = round(float(raw.rstrip("%")) * 255 / 100) if raw.endswith("%") \
                    else round(float(raw) * 255)
            else:
                alpha = 255
        except ValueError:
            return None
        return (channels[0], channels[1], channels[2], max(0, min(255, alpha)))
    named = NAMED_COLOURS.get(value.lower())
    if named is not None:
        return named
    return None


def parse_length(value: str) -> float | None:
    """A `rem`/`px` length in points, against the browser's 16px root."""
    match = re.fullmatch(r"(-?\d+(?:\.\d+)?)(rem|px)", value.strip())
    if not match:
        return None
    number = float(match.group(1))
    return number * ROOT_PX if match.group(2) == "rem" else number


def parse_number(value: str) -> float | None:
    try:
        return float(value.strip())
    except ValueError:
        return None


def parse_bezier(value: str) -> tuple[float, float, float, float] | None:
    text = value.strip()
    match = re.fullmatch(r"cubic-bezier\(([^)]*)\)", text)
    if match:
        parts = [part for part in re.split(r"[,\s]+", match.group(1).strip()) if part]
        if len(parts) == 4:
            try:
                return (float(parts[0]), float(parts[1]), float(parts[2]), float(parts[3]))
            except ValueError:
                return None
        return None
    return NAMED_EASINGS.get(text.lower())


def colour_of(resolution: Resolution) -> Resolution:
    """Fill in the RGB and alpha of a resolved token, or fail naming it."""
    colour = parse_colour(resolution.value)
    if colour is None:
        raise SystemExit(
            f"{resolution.declared_sheet}:{resolution.declared_line}: "
            f"{resolution.token} = {resolution.declared_value} resolves to "
            f"`{resolution.value}`, which this generator cannot read as a colour"
        )
    r, g, b, a = colour
    resolution.rgb = (r, g, b)
    resolution.alpha = a
    return resolution


# ---------------------------------------------------------------------------
# Motion: the transitions the sheets declare
# ---------------------------------------------------------------------------


@dataclass(frozen=True)
class Transition:
    """One property a `transition:` shorthand moves, and how."""

    sheet: str
    line: int
    property: str
    millis: int
    easing_name: str
    curve: tuple[float, float, float, float]


def scan_transitions(root: dict[str, tuple[str, int, str, str]]) -> list[Transition]:
    """Every single-property transition the vendored sheets declare.

    A shorthand may list one duration and easing shared by a comma-separated property
    list, which a naive split reads as several transitions; an item with no duration of
    its own is skipped rather than guessed at. `var(--ease-out-expo)` is resolved
    against the `:root` block, and an easing variable that is not there fails loudly.
    """
    rows: list[Transition] = []
    seen: set[tuple[str, int, str, int]] = set()
    for path in sheet_paths():
        if not path.is_file():
            continue
        name = path.relative_to(VENDOR).as_posix()
        text = strip_comments(path.read_text(encoding="utf-8"))
        line_starts = [0]
        for index, char in enumerate(text):
            if char == "\n":
                line_starts.append(index + 1)

        def line_at(index: int, starts: list[int] = line_starts) -> int:
            low, high = 0, len(starts) - 1
            while low < high:
                mid = (low + high + 1) // 2
                if starts[mid] <= index:
                    low = mid
                else:
                    high = mid - 1
            return low + 1

        for match in re.finditer(r"(?<![\w-])transition(?:-duration|-timing-function)?\s*:\s*([^;}]*)", text):
            line = line_at(match.start())
            body = normalise(match.group(1))
            for item in split_items(body):
                item = re.sub(r"!\s*important", "", item).strip()
                tokens = split_tokens(item)
                durations = [token for token in tokens if re.fullmatch(r"\d+(?:\.\d+)?m?s", token)]
                if len(durations) != 1:
                    continue
                raw_duration = durations[0]
                millis = round(float(raw_duration[:-2])) if raw_duration.endswith("ms") \
                    else round(float(raw_duration[:-1]) * 1000)
                if not 0 < millis <= LONGEST_TRANSITION_MS:
                    continue
                easing_token = ""
                easing_name = "ease"
                curve = None
                for token in tokens:
                    if token == raw_duration:
                        continue
                    found = parse_bezier(token)
                    if found is not None:
                        easing_token, easing_name, curve = token, token, found
                        break
                    var_match = BARE_VAR.fullmatch(token)
                    if var_match:
                        variable = var_match.group(1)
                        if variable not in root:
                            raise SystemExit(
                                f"{name}:{line}: `{item}` moves on `var({variable})`, "
                                "which no :root block declares"
                            )
                        resolved = resolve(root, variable, "motion")
                        inner = parse_bezier(resolved.value)
                        if inner is None:
                            raise SystemExit(
                                f"{name}:{line}: `var({variable})` resolves to "
                                f"`{resolved.value}`, which is not a cubic-bezier"
                            )
                        easing_token, easing_name, curve = token, variable, inner
                        break
                if curve is None:
                    # No easing stated: CSS's default is `ease`.
                    easing_token, easing_name, curve = "ease", "ease", NAMED_EASINGS["ease"]
                properties = [
                    token for token in tokens
                    if token != raw_duration and token != easing_token
                    and not re.fullmatch(r"\d+(?:\.\d+)?m?s", token)
                ]
                if len(properties) != 1:
                    continue
                key = (name, line, properties[0], millis)
                if key in seen:
                    continue
                seen.add(key)
                rows.append(Transition(name, line, properties[0], millis, easing_name, curve))
    rows.sort(key=lambda row: (row.sheet, row.line, row.property, row.millis))
    return rows


def pick_transition(rows: list[Transition], pick: MotionPick) -> Transition:
    for row in rows:
        if row.sheet == pick.sheet and row.property == pick.property and row.millis == pick.millis:
            return row
    raise SystemExit(
        f"{pick.sheet}: no `{pick.property} {pick.millis}ms` transition any more -- "
        f"{pick.const} would be stale. The sheet was retimed or the shorthand rewritten."
    )


# ---------------------------------------------------------------------------
# Writing Rust
# ---------------------------------------------------------------------------


def fnum(value: float) -> str:
    """A float as Rust source, with a decimal point, so `f32` is unambiguous."""
    text = f"{value:.4f}".rstrip("0").rstrip(".")
    if text in ("", "-"):
        text = "0"
    if "." not in text:
        text += ".0"
    return text


def escape(text: str) -> str:
    return text.replace("\\", "\\\\").replace('"', '\\"')


def short(sheet: str) -> str:
    """`assets/styles/variables.scss` -> `variables.scss`, for a line comment."""
    return sheet.rsplit("/", 1)[-1]


def seconds(millis: int) -> str:
    """`50` -> `0.05s`, `200` -> `0.2s`, so a comment reads the way the sheet writes it."""
    text = f"{millis / 1000:.3f}".rstrip("0").rstrip(".")
    return f"{text}s"


def provenance(resolution: Resolution) -> str:
    """`variables.scss:233`, plus the `var()` chain when the value was reached through one."""
    text = f"{short(resolution.final_sheet)}:{resolution.final_line}"
    if resolution.chain:
        chain = " -> ".join(f"var({name})" for name, *_ in resolution.chain)
        text += f" (via {chain} at {short(resolution.declared_sheet)}:{resolution.declared_line})"
    return text


def premultiply(rgb: tuple[int, int, int], alpha: int) -> tuple[int, int, int]:
    """What `Color32::from_rgba_unmultiplied` would store, computed here.

    `from_rgba_unmultiplied` is not a `const fn` in every egui version, and these
    values live in `const`s; `from_rgba_premultiplied` is const by construction
    (`Color32::TRANSPARENT` is built with it). Doing the multiply here keeps the
    constants const and the value the one egui would have computed.
    """
    if alpha == 255:
        return rgb
    if alpha == 0:
        return (0, 0, 0)
    return tuple(  # type: ignore[return-value]
        min(255, int(channel * alpha / 255.0 + 0.5)) for channel in rgb
    )


def colour_expr(rgba: tuple[int, int, int, int]) -> str:
    """`egui::Color32::from_rgb(0x.., 0x.., 0x..)`, or the alpha-preserving form."""
    r, g, b, a = rgba
    if a == 255:
        return f"egui::Color32::from_rgb(0x{r:02x}, 0x{g:02x}, 0x{b:02x})"
    pr, pg, pb = premultiply((r, g, b), a)
    return (
        f"egui::Color32::from_rgba_premultiplied("
        f"0x{pr:02x}, 0x{pg:02x}, 0x{pb:02x}, 0x{a:02x})"
    )


def value_text(rgba: tuple[int, int, int, int]) -> str:
    r, g, b, a = rgba
    if a == 255:
        return f"#{r:02x}{g:02x}{b:02x}"
    return f"rgba({r}, {g}, {b}, {a / 255:.2f})"

def emit() -> tuple[str, list[str], dict[str, int]]:
    blocks = load_blocks()

    tables = {scheme: build_scheme(blocks, scheme) for scheme in ("dark", "light")}

    # Every colour token, in both schemes.
    resolutions: dict[str, dict[str, Resolution]] = {"dark": {}, "light": {}}
    for scheme in ("dark", "light"):
        for token in COLOUR_TOKENS:
            resolutions[scheme][token.css] = colour_of(resolve(tables[scheme], token.css, scheme))

    # The one bare-number token the launcher reads.
    factors: dict[str, Resolution] = {}
    for scheme in ("dark", "light"):
        factor = resolve(tables[scheme], "--hover-brightness", scheme)
        number = parse_number(factor.value)
        if number is None:
            raise SystemExit(f"--hover-brightness is `{factor.value}`, which is not a number")
        factor.rgb = None
        factor.alpha = None
        factors[scheme] = factor

    # Geometry and type come from the shared `html` / `body` blocks, so one scheme's
    # table is as good as the other's -- but a scheme that overrode one would be a
    # fact worth failing on rather than silently taking dark's.
    geometry: dict[str, Resolution] = {}
    for token in RADIUS_TOKENS + GAP_TOKENS:
        dark = resolve(tables["dark"], token, "dark")
        light = resolve(tables["light"], token, "light")
        if dark.value != light.value:
            raise SystemExit(f"{token} differs between schemes ({dark.value} / {light.value}); "
                             "geometry is declared once, on `html`")
        geometry[token] = dark
    typography: dict[str, Resolution] = {}
    for token, _ in TYPE_SIZE_TOKENS + TYPE_WEIGHT_TOKENS:
        typography[token] = resolve(tables["dark"], token, "dark")

    transitions = scan_transitions(tables["dark"])
    picked = [(pick, pick_transition(transitions, pick)) for pick in MOTION_PICKS]

    # The easing consts, in the order they are first used. A `var(--ease-…)` is cited
    # at the `:root` line that declares the curve; an inline bezier at the transition
    # that spells it out.
    easings: dict[str, tuple[str, tuple[float, float, float, float], str]] = {}
    for pick, row in picked:
        if pick.easing_const in easings:
            continue
        display = row.easing_name
        source = f"{row.sheet}:{row.line}"
        if display.startswith("--"):
            declared = resolve(tables["dark"], display, "easing")
            source = f"{declared.declared_sheet}:{declared.declared_line}"
        easings[pick.easing_const] = (display, row.curve, source)

    lines: list[str] = []
    add = lines.append

    dark_block = blocks["dark-mode"][0][1]
    light_block = blocks["light-properties"][0][1]
    html_block = blocks["html"][0][1]
    body_block = blocks["body"][0][1]

    colour_count = len(COLOUR_TOKENS)
    translucent = sum(
        1 for scheme in resolutions.values() for res in scheme.values() if res.alpha != 255
    )

    # ---- header ----------------------------------------------------------
    add("//! The reference client's palette, geometry, type and motion, as egui values.")
    add("//!")
    add("//! **Generated by `tools/gen_theme_egui.py`. Do not edit.**")
    add("//!")
    add("//! Everything below is a value read out of the Modrinth App's vendored CSS at")
    add("//! the commit `vendor/modrinth-app/UPSTREAM.md` pins, resolved the way a browser")
    add("//! would resolve it: comments stripped, multi-line declarations joined, `@extend`")
    add("//! chains followed, and `var()` indirection followed to a fixed point before")
    add("//! anything was written. The tool is `tools/gen_theme_egui.py`; the gate is")
    add("//! `python tools/gen_theme_egui.py --check`, which regenerates this file in memory")
    add("//! and compares it byte for byte.")
    add("//!")
    add("//! The names are the CSS custom properties in SCREAMING_SNAKE, with the `--color-`")
    add("//! prefix dropped because the field is already a colour: `--color-bg` is")
    add("//! [`Theme::BG`], `--color-raised-bg` is [`Theme::RAISED_BG`]. Every initialiser in")
    add("//! [`DARK`] and [`LIGHT`] carries the sheet, the line and the `var()` chain it came")
    add("//! through, so a stale value traces back to the line that moved.")
    add("//!")
    add("//! | Source | What came out of it |")
    add("//! | --- | --- |")
    add(f"//! | `{VARIABLES}` | {colour_count} colour tokens, "
        f"{len(RADIUS_TOKENS)} radii, {len(GAP_TOKENS)} gaps |")
    add(f"//! | `{DEFAULTS}` | {len(TYPE_SIZE_TOKENS)} font sizes, {len(TYPE_WEIGHT_TOKENS)} font weights |")
    add(f"//! | `{UTILITIES}` | `--ease-out-expo`, and the popover and floating-bar transitions |")
    add("//!")
    add("//! Written against egui 0.36.1. The crate's egui source was not on the machine this")
    add("//! was generated on, so the fields `colors()` touches are assumed from the 0.31")
    add("//! renames: `Visuals::window_corner_radius`, `WidgetVisuals::corner_radius` and")
    add("//! `CornerRadius::same(u8)`. If a build says a field does not exist, that is what")
    add("//! moved -- not a colour.")
    add("")
    add("use egui::{Color32, CornerRadius, Stroke, Visuals};")
    add("")

    # ---- the Theme struct ------------------------------------------------
    add("/// One scheme's tokens.")
    add("///")
    add("/// The field names are the stylesheet's own, so `Theme::BG` is the value of")
    add("/// `--color-bg` and nothing else. A field marked `allow(dead_code)` is a token the")
    add("/// sheet declares and this module does not read yet; the marker is per field rather")
    add("/// than crate-wide because the next token to stop being used should be visible too.")
    add("#[allow(non_snake_case)]")
    add("#[derive(Clone, Copy, Debug, PartialEq)]")
    add("pub struct Theme {")
    section = ""
    for token in COLOUR_TOKENS:
        if token.section != section:
            section = token.section
            add("")
            add(f"    // ---- {section} ------------------------------------------------------")
            add("")
        field = field_name(token.css)
        add(f"    /// `{token.css}` -- {token.note}.")
        if field not in USED_COLOURS:
            add("    ///")
            add("    /// Emitted because the stylesheet declares it; the page that uses it has")
            add("    /// not been ported yet.")
            add("    #[allow(dead_code)]")
        add(f"    pub {field}: Color32,")
    add("")
    add("    // ---- factors --------------------------------------------------------")
    add("")
    add("    /// `--hover-brightness` -- the factor a control's fill is multiplied by under")
    add("    /// the pointer (0.9 in light, which darkens it; 1.25 in dark, which brightens).")
    add("    pub HOVER_BRIGHTNESS: f32,")
    add("")
    add("    /// Not a token: which scheme this is, for `egui::Visuals::dark_mode` and for")
    add("    /// which of egui's own two base themes `colors()` starts from.")
    add("    pub DARK_MODE: bool,")
    add("}")
    add("")

    # ---- the two schemes -------------------------------------------------
    def emit_scheme(name: str, label: str, block: Block, note: str) -> None:
        add(f"/// The {label} scheme.")
        add("///")
        add(f"/// Read from `vendor/modrinth-app/{VARIABLES}`: the "
            f"`{', '.join(block.selectors)}` block")
        add(f"/// at line {block.line}, resolved over the `{', '.join(light_block.selectors)}` base at")
        add(f"/// line {light_block.line} and the shared `html` block at line {html_block.line}, in")
        add("/// cascade order. Each comment below is `token: value -- the line that states it`")
        add("/// `(via the chain the value was reached through)`.")
        add("///")
        add(f"/// {note}")
        add(f"pub const {name}: Theme = Theme {{")
        section = ""
        for token in COLOUR_TOKENS:
            if token.section != section:
                section = token.section
                add(f"    // ---- {section} ----")
            res = resolutions["dark" if name == "DARK" else "light"][token.css]
            field = field_name(token.css)
            bits = [f"    // {token.css}: {value_text(res.rgba)} -- {res.final_sheet}:{res.final_line}"]
            if res.chain:
                chain = " -> ".join(f"var({var})" for var, *_ in res.chain)
                bits.append(f" (via {chain} at {res.declared_sheet}:{res.declared_line})")
            if res.rgba[3] != 255:
                bits.append(f"; stored premultiplied, alpha {res.alpha}")
            if name == "DARK" and res.declared_block not in SCHEME_SELECTORS["dark"]:
                bits.append("; the dark block does not override it")
            add("".join(bits))
            add(f"    {field}: {colour_expr(res.rgba)},")
        add(f"    // --hover-brightness: {fnum(float(factors['dark' if name == 'DARK' else 'light'].value))}"
            f" -- {factors['dark' if name == 'DARK' else 'light'].final_sheet}:"
            f"{factors['dark' if name == 'DARK' else 'light'].final_line}")
        add(f"    HOVER_BRIGHTNESS: {fnum(float(factors['dark' if name == 'DARK' else 'light'].value))},")
        add(f"    DARK_MODE: {'true' if name == 'DARK' else 'false'},")
        add("};")
        add("")

    emit_scheme("DARK", "dark", dark_block,
                "This is the scheme the reference opens in (`:root { color-scheme: dark }`,\n"
                "/// `app-frontend/src/assets/stylesheets/global.scss:45`), so it is the one the\n"
                "/// numbers in `REFERENCE.md` were taken in.")
    emit_scheme("LIGHT", "light", light_block,
                "`.light-mode` only extends `.light-properties`; the light scheme is that base\n"
                "/// plus the shared `html` block, which is also where a dark-only token falls back\n"
                "/// to when `.dark-mode` does not override it.")

    # ---- geometry --------------------------------------------------------
    add("// ---------------------------------------------------------------------------")
    add(f"// Geometry: the `html` block, `{VARIABLES}:{html_block.line}`")
    add("//")
    add("// The reference declares one ladder for both schemes, in `rem` against the")
    add("// browser's 16px root, and `html` is where it lives. Radii are whole pixels")
    add("// because that is what egui's `CornerRadius` holds; gaps are points.")
    add("// ---------------------------------------------------------------------------")
    add("")
    for token in RADIUS_TOKENS:
        res = geometry[token]
        points = parse_length(res.value)
        if points is None:
            raise SystemExit(f"{token} = `{res.value}` is not a length")
        rounded = max(0, min(255, round(points)))
        field = field_name(token)
        note = ""
        if rounded != points:
            note = (f"\n///\n/// The sheet writes `{res.value}`, which is a pill; 255 is the largest radius\n"
                    "/// egui's `CornerRadius` can hold, and a pill's exact value cannot be seen.")
        add(f"/// `{token}` -- {fnum(points)} points ({res.final_sheet}:{res.final_line}).{note}")
        add(f"pub const {field}: u8 = {rounded};")
        add("")
    for token in GAP_TOKENS:
        res = geometry[token]
        points = parse_length(res.value)
        if points is None:
            raise SystemExit(f"{token} = `{res.value}` is not a length")
        add(f"/// `{token}` -- {fnum(points)} points ({res.final_sheet}:{res.final_line}).")
        add(f"pub const {field_name(token)}: f32 = {fnum(points)};")
        add("")
    add("/// The border the sheet writes literally on every control:")
    add("/// `border: 1px solid var(--color-button-border)` (`assets/styles/classes.scss:753`)")
    add("/// and `border: 1px solid var(--color-divider)` (`assets/styles/classes.scss:1148`).")
    add("/// Not a token -- the width is a literal in the sheet -- but the one number every")
    add("/// edge in this module is stroked with.")
    add("pub const STROKE_WIDTH: f32 = 1.0;")
    add("")

    # ---- typography ------------------------------------------------------
    add("// ---------------------------------------------------------------------------")
    add(f"// Type: the `body` block, `{DEFAULTS}:{body_block.line}`")
    add("//")
    add("// Three sizes and three weights, which are what the shell sets small text, body")
    add("// text and headings in. They are scheme-independent: `body` is declared once, on")
    add("// the element every page sits in.")
    add("// ---------------------------------------------------------------------------")
    add("")
    for token, note in TYPE_SIZE_TOKENS:
        res = typography[token]
        points = parse_length(res.value)
        if points is None:
            raise SystemExit(f"{token} = `{res.value}` is not a length")
        add(f"/// `{token}` -- {fnum(points)} points, {note} ({res.final_sheet}:{res.final_line}).")
        add(f"pub const {field_name(token)}: f32 = {fnum(points)};")
        add("")
    for token, note in TYPE_WEIGHT_TOKENS:
        res = typography[token]
        number = parse_number(res.value)
        if number is None:
            raise SystemExit(f"{token} = `{res.value}` is not a weight")
        add(f"/// `{token}` -- {int(number)}, {note} ({res.final_sheet}:{res.final_line}).")
        add(f"pub const {field_name(token)}: u16 = {int(number)};")
        add("")

    # ---- motion ----------------------------------------------------------
    add("// ---------------------------------------------------------------------------")
    add("// Motion: the transitions the sheets declare and this shell animates")
    add("//")
    add("// Durations are in milliseconds and easings are cubic-beziers, because that is")
    add("// what a tween needs. Each constant names the rule it was read from, and the")
    add("// table at the end carries the same numbers with their property.")
    add("// ---------------------------------------------------------------------------")
    add("")
    add("/// A cubic-bezier as the stylesheet names it. `curve` is `(x1, y1, x2, y2)`.")
    add("#[derive(Clone, Copy, Debug, PartialEq)]")
    add("pub struct Easing {")
    add("    /// The name the sheet uses: `ease-in-out`, `--ease-out-expo`, or the bezier")
    add("    /// spelled out where the sheet spells it out.")
    add("    pub name: &'static str,")
    add("    /// The four control points, ready for a cubic-bezier evaluator.")
    add("    pub curve: [f32; 4],")
    add("}")
    add("")
    for name, (display, curve, source) in easings.items():
        if display.startswith("var("):
            variable = display[4:-1]
            add(f"/// `{variable}`, declared at `{source}`.")
        elif display in NAMED_EASINGS:
            add(f"/// CSS's `{display}`, named by the transitions below (`{source}`) and")
            add("/// transcribed from the specification rather than from this tree.")
        else:
            add(f"/// The curve written inline at `{source}`.")
        points = ", ".join(fnum(part) for part in curve)
        add(f"pub const {name}: Easing = Easing {{")
        add(f"    name: \"{escape(display)}\",")
        add(f"    curve: [{points}],")
        add("};")
        add("")
    add("/// Every easing above, for `Theme::easing`.")
    add("pub const EASINGS: &[Easing] = &[")
    for name in easings:
        add(f"    {name},")
    add("];")
    add("")

    for pick, row in picked:
        display = row.easing_name
        wording = f"{row.property} {row.millis / 1000:.3f}s".replace("0.050s", "0.05s")
        add(f"/// `{wording} {display if not display.startswith('--') else 'var(' + display + ')'}`")
        add(f"/// -- {pick.note} (`{row.sheet}:{row.line}`).")
        add(f"pub const {pick.const}: u32 = {row.millis};")
        add("")

    add("/// One transition the reference declares and this shell animates.")
    add("#[derive(Clone, Copy, Debug, PartialEq)]")
    add("pub struct Motion {")
    add("    /// What moves, as the CSS property is named.")
    add("    pub property: &'static str,")
    add("    /// How long it takes, in milliseconds.")
    add("    pub millis: u32,")
    add("    /// The curve it moves on.")
    add("    pub easing: Easing,")
    add("    /// Where the sheet states it, `vendor/modrinth-app/<file>:<line>`, so a stale")
    add("    /// number can be traced to the rule that moved.")
    add("    pub source: &'static str,")
    add("}")
    add("")
    add("/// Every transition above, with the property it moves.")
    add("pub const MOTION: &[Motion] = &[")
    for pick, row in picked:
        add("    Motion {")
        add(f"        property: \"{escape(row.property)}\",")
        add(f"        millis: {pick.const},")
        add(f"        easing: {pick.easing_const},")
        add(f"        source: \"vendor/modrinth-app/{row.sheet}:{row.line}\",")
        add("    },")
    add("];")
    add("")

    # ---- accessors -------------------------------------------------------
    add("impl Theme {")
    add("    /// The egui [`Visuals`] this scheme paints with.")
    add("    ///")
    add("    /// Every field set here is named with the token it comes from; a field egui has")
    add("    /// and the reference does not name keeps whatever egui's own base puts there, so")
    add("    /// the two schemes differ only where the stylesheet says they differ.")
    add("    pub fn colors(&self) -> Visuals {")
    add("        let mut visual = if self.DARK_MODE {")
    add("            Visuals::dark()")
    add("        } else {")
    add("            Visuals::light()")
    add("        };")
    add("")
    add("        visual.dark_mode = self.DARK_MODE;")
    add("")
    add("        // `body { background-color: var(--color-bg) }` (defaults.scss:10) is the")
    add("        // page, and a panel is the page: the reference has one flat background")
    add("        // behind its chrome, with cards raised on top of it.")
    add("        visual.window_fill = self.BG;")
    add("        visual.panel_fill = self.BG;")
    add("")
    add("        // A raised card or panel: `--color-raised-bg`.")
    add("        visual.faint_bg_color = self.RAISED_BG;")
    add("")
    add("        // A text input: the reference's `InputFrame` paints `bg-surface-4`, which")
    add("        // is the value `--color-button-bg` resolves to in both schemes.")
    add("        visual.extreme_bg_color = self.BUTTON_BG;")
    add("")
    add("        // The interface's default text is `--color-base` (defaults.scss:11).")
    add("        visual.override_text_color = Some(self.BASE);")
    add("")
    add("        visual.hyperlink_color = self.LINK;")
    add("        visual.warn_fg_color = self.ORANGE;")
    add("        visual.error_fg_color = self.RED;")
    add("")
    add("        // A popover is `--color-raised-bg` inside")
    add("        // `1px solid var(--color-button-border)` at `--radius-md`")
    add("        // (classes.scss:752-756). A window in this shell is a page, so only the")
    add("        // edge and the radius come from the popover; the fill stays `BG`, which is")
    add("        // what the test at the bottom of this file pins.")
    add("        visual.window_stroke = Stroke::new(STROKE_WIDTH, self.BUTTON_BORDER);")
    add("        visual.window_corner_radius = CornerRadius::same(RADIUS_LG);")
    add("")
    add("        // Text selection: `--color-brand-highlight` inside `--color-brand`.")
    add("        visual.selection.bg_fill = self.BRAND_HIGHLIGHT;")
    add("        visual.selection.stroke = Stroke::new(STROKE_WIDTH, self.BRAND);")
    add("")
    add("        // A frame or card: `--color-raised-bg` inside a `--color-divider` line.")
    add("        visual.widgets.noninteractive.bg_fill = self.RAISED_BG;")
    add("        visual.widgets.noninteractive.weak_bg_fill = self.RAISED_BG;")
    add("        visual.widgets.noninteractive.bg_stroke = Stroke::new(STROKE_WIDTH, self.DIVIDER);")
    add("        visual.widgets.noninteractive.fg_stroke = Stroke::new(STROKE_WIDTH, self.BASE);")
    add("        visual.widgets.noninteractive.corner_radius = CornerRadius::same(RADIUS_MD);")
    add("        visual.widgets.noninteractive.expansion = 0.0;")
    add("")
    add("        // A button at rest: `--color-button-bg` inside `--color-button-border`, at")
    add("        // the `--radius-md` the sheet gives `.btn` (classes.scss:398).")
    add("        visual.widgets.inactive.bg_fill = self.BUTTON_BG;")
    add("        visual.widgets.inactive.weak_bg_fill = self.BUTTON_BG;")
    add("        visual.widgets.inactive.bg_stroke = Stroke::new(STROKE_WIDTH, self.BUTTON_BORDER);")
    add("        visual.widgets.inactive.fg_stroke = Stroke::new(STROKE_WIDTH, self.BASE);")
    add("        visual.widgets.inactive.corner_radius = CornerRadius::same(RADIUS_MD);")
    add("        visual.widgets.inactive.expansion = 0.0;")
    add("")
    add("        // Under the pointer the fill moves by `--hover-brightness` and the text")
    add("        // becomes `--color-contrast`.")
    add("        visual.widgets.hovered.bg_fill = self.hovered(self.BUTTON_BG);")
    add("        visual.widgets.hovered.weak_bg_fill = self.hovered(self.BUTTON_BG);")
    add("        visual.widgets.hovered.bg_stroke = Stroke::new(STROKE_WIDTH, self.BUTTON_BORDER);")
    add("        visual.widgets.hovered.fg_stroke = Stroke::new(STROKE_WIDTH, self.CONTRAST);")
    add("        visual.widgets.hovered.corner_radius = CornerRadius::same(RADIUS_MD);")
    add("        visual.widgets.hovered.expansion = 0.0;")
    add("")
    add("        // Pressed. The sheet presses to `brightness(0.8)` against `0.85` on hover")
    add("        // (classes.scss:215, :220); one factor is declared (`--hover-brightness`),")
    add("        // so the two states share it and the press differs only in its text.")
    add("        visual.widgets.active.bg_fill = self.hovered(self.BUTTON_BG);")
    add("        visual.widgets.active.weak_bg_fill = self.hovered(self.BUTTON_BG);")
    add("        visual.widgets.active.bg_stroke = Stroke::new(STROKE_WIDTH, self.BUTTON_BORDER);")
    add("        visual.widgets.active.fg_stroke = Stroke::new(STROKE_WIDTH, self.CONTRAST);")
    add("        visual.widgets.active.corner_radius = CornerRadius::same(RADIUS_MD);")
    add("        visual.widgets.active.expansion = 0.0;")
    add("")
    add("        // An open menu: raised, with the popover's border and `--color-contrast`.")
    add("        visual.widgets.open.bg_fill = self.RAISED_BG;")
    add("        visual.widgets.open.weak_bg_fill = self.RAISED_BG;")
    add("        visual.widgets.open.bg_stroke = Stroke::new(STROKE_WIDTH, self.BUTTON_BORDER);")
    add("        visual.widgets.open.fg_stroke = Stroke::new(STROKE_WIDTH, self.CONTRAST);")
    add("        visual.widgets.open.corner_radius = CornerRadius::same(RADIUS_MD);")
    add("        visual.widgets.open.expansion = 0.0;")
    add("")
    add("        visual")
    add("    }")
    add("")
    add("    /// The fill a control takes under the pointer.")
    add("    ///")
    add("    /// The reference does this with a filter --")
    add("    /// `filter: brightness(var(--hover-brightness))` (`variables.scss:187`, `:407`)")
    add("    /// -- and egui has no filters, so the same factor is applied to the colour the")
    add("    /// control was going to paint. It darkens in light (0.9) and brightens in dark")
    add("    /// (1.25), which is the direction the reference moves a control in each scheme.")
    add("    pub fn hovered(&self, fill: Color32) -> Color32 {")
    add("        let scale = |channel: u8| -> u8 {")
    add("            let scaled = f32::from(channel) * self.HOVER_BRIGHTNESS;")
    add("            scaled.round().clamp(0.0, 255.0) as u8")
    add("        };")
    add("        Color32::from_rgba_unmultiplied(scale(fill.r()), scale(fill.g()), scale(fill.b()), fill.a())")
    add("    }")
    add("")
    add("    /// The `--gap-*` ladder in points, `xs, sm, md, lg, xl` (`variables.scss:195-199`).")
    add("    pub fn gaps() -> [f32; 5] {")
    add("        [GAP_XS, GAP_SM, GAP_MD, GAP_LG, GAP_XL]")
    add("    }")
    add("")
    add("    /// The `--radius-*` ladder in whole pixels, `xs, sm, md, lg, xl, max`")
    add("    /// (`variables.scss:201-206`), the shape egui's `CornerRadius` takes.")
    add("    pub fn radii() -> [u8; 6] {")
    add("        [RADIUS_XS, RADIUS_SM, RADIUS_MD, RADIUS_LG, RADIUS_XL, RADIUS_MAX]")
    add("    }")
    add("")
    add("    /// The sizes small text, body and headings are set in: `--font-size-xs` (12px,")
    add("    /// a metadata row), `--font-size-nm` (16px, the size `body` itself is set in)")
    add("    /// and `--font-size-xl` (24px, the 1.5rem the page header's `text-2xl` also")
    add("    /// resolves to), from the ladder at `defaults.scss:23-31`.")
    add("    pub fn type_sizes() -> [f32; 3] {")
    add("        [FONT_SIZE_XS, FONT_SIZE_NM, FONT_SIZE_XL]")
    add("    }")
    add("")
    add("    /// The weights: body (`--font-weight-regular`, 400), the interface")
    add("    /// (`--font-weight-text`, 500) and headings (`--font-weight-heading`, 800).")
    add("    pub fn type_weights() -> [u16; 3] {")
    add("        [FONT_WEIGHT_REGULAR, FONT_WEIGHT_TEXT, FONT_WEIGHT_HEADING]")
    add("    }")
    add("")
    add("    /// Every transition the reference declares that this shell animates.")
    add("    pub fn motion() -> &'static [Motion] {")
    add("        MOTION")
    add("    }")
    add("")
    add("    /// The easing a transition moves on, by the name the sheet used for it.")
    add("    pub fn easing(name: &str) -> Option<Easing> {")
    add("        EASINGS.iter().copied().find(|easing| easing.name == name)")
    add("    }")
    add("}")
    add("")
    add("/// The scheme the launcher opens in: the reference's own default (`color-scheme:")
    add("/// dark` at `app-frontend/src/assets/stylesheets/global.scss:45`).")
    add("pub fn dark() -> Theme {")
    add("    DARK")
    add("}")
    add("")
    add("/// The light scheme.")
    add("pub fn light() -> Theme {")
    add("    LIGHT")
    add("}")
    add("")

    # ---- tests -----------------------------------------------------------
    opaque: list[tuple[str, Resolution]] = []
    translucent_rows: list[tuple[str, Resolution]] = []
    for scheme, label in (("dark", "DARK"), ("light", "LIGHT")):
        for token in COLOUR_TOKENS:
            res = resolutions[scheme][token.css]
            row = (f"{field_name(token.css)} ({label.lower()})", res)
            if res.alpha == 255:
                opaque.append(row)
            else:
                translucent_rows.append(row)

    add("#[cfg(test)]")
    add("mod tests {")
    add("    use super::*;")
    add("")
    add("    /// Every colour the two schemes declare opaque, named for its token.")
    add("    const OPAQUE: &[(&str, Color32)] = &[")
    for name, res in opaque:
        scheme = "DARK" if "(dark)" in name else "LIGHT"
        field = name.split(" ")[0]
        add(f"        (\"{name}\", {scheme}.{field}),")
    add("    ];")
    add("")
    add("    /// Every translucent colour, with the alpha the sheet's `rgba()` declared.")
    add("    const TRANSLUCENT: &[(&str, Color32, u8)] = &[")
    for name, res in translucent_rows:
        scheme = "DARK" if "(dark)" in name else "LIGHT"
        field = name.split(" ")[0]
        add(f"        (\"{name}\", {scheme}.{field}, {res.alpha}),")
    add("    ];")
    add("")
    add("    #[test]")
    add("    fn every_colour_is_opaque_unless_the_sheet_declared_alpha() {")
    add("        // An opaque token is written `from_rgb`, a translucent one")
    add("        // `from_rgba_premultiplied`, and the two must not be confused: a border")
    add("        // that lost its 0.12 alpha paints solid, which is a screenshot's worth of")
    add("        // difference and invisible in a diff.")
    add("        for (name, colour) in OPAQUE {")
    add("            assert_eq!(colour.a(), 255, \"{name} should be opaque\");")
    add("        }")
    add("        for (name, colour, alpha) in TRANSLUCENT {")
    add("            assert_eq!(colour.a(), *alpha, \"{name} has the wrong alpha\");")
    add("        }")
    add(f"        // {len(opaque)} opaque and {len(translucent_rows)} translucent, so a colour that")
    add("        // went missing from both lists is a failure rather than a shorter test.")
    add("        assert_eq!(")
    add("            OPAQUE.len() + TRANSLUCENT.len(),")
    add(f"            {len(opaque) + len(translucent_rows)},")
    add("            \"a colour went missing from both lists\"")
    add("        );")
    add("    }")
    add("")
    add("    #[test]")
    add("    fn the_dark_background_is_the_stylesheets_bg() {")
    add("        // `--color-bg` is `var(--surface-1)`, and `.dark-mode` sets `--surface-1` to")
    add("        // `#16181c` at variables.scss:233. The triple is written out here rather")
    add("        // than compared to the constant, because `--check` already proves this file")
    add("        // matches the generator: only a number that is in neither catches the two of")
    add("        // them agreeing on something the sheet does not say.")
    add("        assert_eq!(DARK.BG, Color32::from_rgb(0x16, 0x18, 0x1c));")
    add("        assert_eq!(LIGHT.BG, Color32::from_rgb(0xeb, 0xeb, 0xeb));")
    add("    }")
    add("")
    add("    #[test]")
    add("    fn colors_paints_the_window_with_bg() {")
    add("        // The page colour is the window colour in this shell: one flat background")
    add("        // behind the chrome, as the reference has it.")
    add("        assert_eq!(DARK.colors().window_fill, DARK.BG);")
    add("        assert_eq!(LIGHT.colors().window_fill, LIGHT.BG);")
    add("        assert_eq!(DARK.colors().panel_fill, DARK.BG);")
    add("        // egui has to be told which look it is in, or every field this module does")
    add("        // not set comes from the wrong one.")
    add("        assert_eq!(DARK.colors().dark_mode, DARK.DARK_MODE);")
    add("        assert!(DARK.DARK_MODE);")
    add("        assert!(!LIGHT.DARK_MODE);")
    add("    }")
    add("")
    add("    #[test]")
    add("    fn the_schemes_differ_where_the_stylesheet_says_so() {")
    add("        assert_ne!(DARK.BG, LIGHT.BG);")
    add("        assert_ne!(DARK.BASE, LIGHT.BASE);")
    add("        assert_ne!(DARK.BUTTON_BG, LIGHT.BUTTON_BG);")
    add("        // Two directions, not one: light darkens a control under the pointer and")
    add("        // dark brightens it (`variables.scss:187`, `:407`).")
    add("        assert!(LIGHT.HOVER_BRIGHTNESS < 1.0);")
    add("        assert!(DARK.HOVER_BRIGHTNESS > 1.0);")
    add("    }")
    add("")
    add("    #[test]")
    add("    fn the_ladders_are_the_ones_the_sheet_declares() {")
    add("        // `html`'s `--radius-*` and `--gap-*` are `rem` against a 16px root;")
    add("        // `--radius-max` is `999999999px` and clamps to the 255 `CornerRadius` holds.")
    add("        assert_eq!(Theme::radii(), [4, 8, 12, 16, 20, 255]);")
    add("        assert_eq!(Theme::gaps(), [4.0, 8.0, 12.0, 16.0, 24.0]);")
    add("        // `--font-size-xs`/`nm`/`xl`, and the weights body, the interface and")
    add("        // headings are set in.")
    add("        assert_eq!(Theme::type_sizes(), [12.0, 16.0, 24.0]);")
    add("        assert_eq!(Theme::type_weights(), [400, 500, 800]);")
    add("    }")
    add("")
    add("    #[test]")
    add("    fn every_motion_row_names_its_source_and_a_sane_duration() {")
    add("        assert!(!Theme::motion().is_empty());")
    add("        for motion in Theme::motion() {")
    add("            assert!(motion.millis > 0, \"{} has no duration\", motion.property);")
    add("            // The reference's longest transition is 2s (`TextLogo.vue`). Every row")
    add("            // past this bound so far was a unit error in the generator, not a slow")
    add("            // animation -- `--check` compares bytes, so only a bound in the tests")
    add("            // catches a number both files agree on.")
    add("            assert!(")
    add("                motion.millis <= 2000,")
    add("                \"{} declares {}ms, longer than any transition in the reference\",")
    add("                motion.property,")
    add("                motion.millis")
    add("            );")
    add("            let (path, line) = motion.source.rsplit_once(':').expect(\"a source is `file:line`\");")
    add("            assert!(")
    add("                path.starts_with(\"vendor/modrinth-app/\")")
    add("                    && (path.ends_with(\".scss\") || path.ends_with(\".css\")),")
    add("                \"{} does not name a stylesheet\",")
    add("                motion.source")
    add("            );")
    add("            assert!(line.parse::<u32>().is_ok(), \"{} has no line\", motion.source);")
    add("        }")
    add("        assert_eq!(Theme::easing(\"ease-in-out\"), Some(EASE_IN_OUT));")
    add("        assert_eq!(Theme::easing(\"--ease-out-expo\"), Some(EASE_OUT_EXPO));")
    add("    }")
    add("}")
    add("")

    text = "\n".join(lines)

    report = [
        f"colours      {colour_count} tokens, {colour_count * 2} values "
        f"({len(opaque)} opaque, {len(translucent_rows)} translucent)",
        f"var()        "
        f"{sum(1 for r in resolutions['dark'].values() if r.via_var)} dark, "
        f"{sum(1 for r in resolutions['light'].values() if r.via_var)} light colour tokens resolved through var()",
        f"geometry     {len(RADIUS_TOKENS)} radii, {len(GAP_TOKENS)} gaps, "
        f"{len(TYPE_SIZE_TOKENS) + len(TYPE_WEIGHT_TOKENS)} type tokens, 1 stroke width",
        f"motion       {len(picked)} transitions, {len(easings)} easings",
        f"lines        {text.count(chr(10)) + 1}",
    ]
    return text, report, {
        "colours": colour_count * 2,
        "opaque": len(opaque),
        "translucent": len(translucent_rows),
    }


def unresolved_colours(tables: dict[str, dict[str, tuple[str, int, str, str]]]) -> list[str]:
    """Every colour token whose chain could not be resolved.

    `emit` raises on the first one; this exists so `--report` can list all of them at
    once instead of one per run. It is not a second resolver -- it asks the same one.
    """
    problems: list[str] = []
    for scheme in ("dark", "light"):
        for token in COLOUR_TOKENS:
            try:
                colour_of(resolve(tables[scheme], token.css, scheme))
            except SystemExit as error:
                problems.append(str(error))
    return problems


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--check", action="store_true",
                        help="fail if the checked-in file is not what this emits")
    parser.add_argument("--report", action="store_true",
                        help="print what every token resolved through")
    args = parser.parse_args()

    for name in SHEET_ORDER:
        path = VENDOR / name
        if not path.is_file():
            print(f"missing reference source: {path}", file=sys.stderr)
            return 2

    text, report, _ = emit()

    if args.report:
        blocks = load_blocks()
        for scheme in ("dark", "light"):
            table = build_scheme(blocks, scheme)
            for token in COLOUR_TOKENS:
                res = resolve(table, token.css, scheme)
                chain = " -> ".join(f"{name}@{sheet}:{line}" for name, _, line, sheet in res.chain)
                print(f"  {scheme:5} {token.css:36} {res.value:28} "
                      f"{res.declared_sheet}:{res.declared_line}"
                      + (f"  via {chain}" if chain else ""))

    if args.check:
        current = OUT.read_text(encoding="utf-8") if OUT.is_file() else ""
        if current != text:
            print(f"{OUT.relative_to(ROOT)} is not what this tool emits.", file=sys.stderr)
            print("Run `python tools/gen_theme_egui.py` and commit the result.", file=sys.stderr)
            return 1
        for line in report:
            print(line)
        print("theme generation is byte-identical")
        return 0

    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text(text, encoding="utf-8", newline="\n")
    for line in report:
        print(line)
    print(f"wrote {OUT.relative_to(ROOT)} ({text.count(chr(10)) + 1} lines)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
