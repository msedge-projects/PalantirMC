#!/usr/bin/env python3
"""Emit the reference's full token vocabulary as Rust tables.

Reads vendor/modrinth-app's own sheets at the commit UPSTREAM.md pins and
writes crates/palantir-desktop/src/theme_tokens.rs: every --token the
reference declares, per mode, resolved through var() chains, classified as a
colour, a length, a number, a gradient's stops, or raw text where the
reference's own syntax (hsla() shadows, font stacks) has no Rust answer here.

This is a generator, not a checker. The checker is the Rust test
`the_generated_vocabulary_is_the_sheets` (reference_tokens.rs), which re-reads
the sheets itself and fails when this file's output and the sheets disagree --
so a stale theme_tokens.rs fails CI with the file and line the reference
states the token on, and the fix is to run this and commit the result. Two
independent readers of the same sheets agreeing is the check; neither one is
trusted alone.

The cascade is the reference's own, as the sheets declare it:

    .light-properties            the light base
    html       @extend light     --gap-*, --radius-*, the ad colours, the ring
    body                         the type ladder and weights (defaults.scss)
    .dark-mode/.dark/:root[dark] overrides for dark
    .oled-mode @extend dark      overrides OLED
    .retro-mode @extend dark     overrides retro

so a mode is: base, then its block. That is what "the reference declares this
token" means in dark mode, and the Rust checker builds the same cascade from
the same files rather than taking this file's word for it.

Run from the repository root:

    python tools/gen_tokens.py

Output is deterministic: same sheets, same bytes. Commit the generated file
with whatever change made it necessary.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]
TREE = REPO / "vendor" / "modrinth-app"
VARIABLES = TREE / "assets" / "styles" / "variables.scss"
DEFAULTS = TREE / "assets" / "styles" / "defaults.scss"
UPSTREAM = TREE / "UPSTREAM.md"
OUT = REPO / "crates" / "palantir-desktop" / "src" / "theme_tokens.rs"

# The reference's root font size, the same line the --font-size-* ladder is
# declared on: `body { font-size: 16px }` in defaults.scss.
REM = 16.0

# `var(--x)` and nothing else -- a bare variable, possibly with a fallback
# after a comma the resolver ignores because every fallback in these sheets is
# never taken.
BARE_VAR = re.compile(r"^var\(\s*(--[\w-]+)\s*(?:,.*)?\)$")

# The two sheets the mode tables are built from. Everything else in the tree
# that declares a token goes in `SCOPED` instead: those declarations belong to a
# selector, not to a mode.
GLOBAL_SHEETS = {VARIABLES.resolve(), DEFAULTS.resolve()}

# A token declaration can only live in a stylesheet or an SFC style block. The
# list is wider than the tree currently uses (there is no `.sass` or `.less`
# here) so that a sheet upstream adds one of those in is walked and fails as a
# stale copy, rather than being invisible and passing.
STYLE_SUFFIXES = (".scss", ".css", ".sass", ".less", ".styl", ".vue")


def upstream_commit() -> str:
    """The commit UPSTREAM.md pins, for the generated file's header."""
    for line in UPSTREAM.read_text(encoding="utf-8").splitlines():
        found = re.search(r"\|\s*Commit\s*\|\s*`([0-9a-f]{40})`", line)
        if found:
            return found.group(1)
    return "unknown commit"


def strip_comment(line: str) -> str:
    """Drop a `//` comment, but not the `//` of a URL."""
    index = line.find("//")
    if index >= 0 and (index == 0 or line[index - 1] != ":"):
        line = line[:index]
    return line


class Block:
    """One selector's declarations, each with the line it starts on."""

    def __init__(self, selector: str) -> None:
        self.selector = selector
        self.tokens: dict[str, tuple[str, int]] = {}
        self.parent: str | None = None


def parse_blocks(text: str) -> list[Block]:
    """Every top-level block, in file order.

    Deliberately not a SCSS parser: these two sheets are one selector plus a
    list of declarations, and the only shapes beyond that are a selector split
    over several lines, a value split over several lines, `@extend`, and (in
    defaults.scss only) nested blocks whose declarations this must not
    collect. Depth tracking covers the nesting; the pending value covers the
    line splits; everything else is a declaration.
    """
    blocks: list[Block] = []
    selector = ""
    depth = 0
    current: Block | None = None
    pending: tuple[str, str, int] | None = None
    in_block_comment = False

    for lineno, raw in enumerate(text.splitlines(), start=1):
        line = strip_comment(raw).strip()
        if in_block_comment:
            if "*/" in line:
                line = line.split("*/", 1)[1].strip()
                in_block_comment = False
            else:
                continue
        if "/*" in line and "*/" not in line:
            in_block_comment = True
            continue
        if not line:
            continue

        # A value that ran onto the next line: join, and keep joining until a
        # `;` shows up. The join is a single space, which is what the Rust
        # reader does, because the two have to agree byte for byte.
        if pending is not None:
            name, value, at = pending
            value = f"{value} {line}"
            assert current is not None
            if ";" in value:
                current.tokens[name] = (value, at)
                pending = None
            else:
                pending = (name, value, at)
            continue

        opens = line.endswith("{")
        if opens:
            if depth == 0:
                selector = f"{selector} {line}".strip() if selector else line
                block_selector = selector[:-1].strip()
            depth += 1
            if depth == 1:
                current = Block(block_selector)
                blocks.append(current)
            selector = ""
            continue

        if depth == 0:
            selector = f"{selector} {line}".strip() if selector else line
            continue

        if line == "}":
            depth -= 1
            if depth == 0:
                current = None
                selector = ""
            continue

        if depth == 1 and line.startswith("--"):
            name, _, value = line.partition(":")
            name, value = name.strip(), value.strip()
            assert current is not None
            if ";" in value:
                current.tokens[name] = (value, lineno)
            else:
                pending = (name, value, lineno)
            continue

        if depth == 1 and line.startswith("@extend"):
            assert current is not None
            current.parent = line[len("@extend"):].strip().rstrip(";").strip()
            continue

        # Any other declaration (`background-color:`, `font-family:`) is a
        # property, not a token, and does not go in the vocabulary.
    return blocks


def cut_value(raw: str) -> str:
    """A declaration's value, without its `;`, its `!important`, or comments."""
    text = raw.split(";")[0].strip()
    text = text.split("//")[0].strip()
    if text.endswith("!important"):
        text = text[: -len("!important")].strip()
    return text


def parse_color(text: str) -> tuple[int, int, int, float] | None:
    """`#rgb`, `#rrggbb`, `#rrggbbaa`, `rgb()`, `rgba()`, or a keyword."""
    text = text.strip()
    if text.startswith("#"):
        digits = text[1:]
        try:
            if len(digits) == 3:
                channels = [int(c * 2, 16) for c in digits]
                return channels[0], channels[1], channels[2], 1.0
            if len(digits) in (6, 8):
                channels = [int(digits[i : i + 2], 16) for i in range(0, len(digits), 2)]
                alpha = channels[3] / 255.0 if len(digits) == 8 else 1.0
                return channels[0], channels[1], channels[2], alpha
        except ValueError:
            return None
        return None
    for prefix, count in (("rgba(", 4), ("rgb(", 3)):
        if text.startswith(prefix) and text.endswith(")"):
            parts = [part.strip() for part in text[len(prefix) : -1].split(",")]
            if len(parts) != count:
                return None
            try:
                numbers = [float(part) for part in parts]
            except ValueError:
                return None
            channels = [int(number / 255.0 * 255.0 + 0.5) for number in numbers[:3]]
            alpha = numbers[3] if count == 4 else 1.0
            return channels[0], channels[1], channels[2], alpha
    keywords = {"white": (255, 255, 255, 1.0), "black": (0, 0, 0, 1.0), "transparent": (0, 0, 0, 0.0)}
    return keywords.get(text)


def split_top_level(inner: str) -> list[str]:
    """Split on the commas that are not inside parentheses."""
    parts: list[str] = []
    depth = 0
    start = 0
    for index, character in enumerate(inner):
        if character == "(":
            depth += 1
        elif character == ")":
            depth = max(0, depth - 1)
        elif character == "," and depth == 0:
            parts.append(inner[start:index])
            start = index + 1
    parts.append(inner[start:])
    return parts


def parse_gradient(text: str) -> list[tuple[tuple[int, int, int, float], float]] | None:
    """The stops of the plain multi-stop form these sheets write."""
    open_at = text.find("(")
    if open_at < 0:
        return None
    inner = text[open_at + 1 : text.rfind(")")]
    stops: list[tuple[tuple[int, int, int, float], float]] = []
    for part in split_top_level(inner):
        part = part.strip()
        if part.startswith(("to ", "from ")) or part.endswith("deg"):
            continue
        pieces = part.rsplit(" ", 1)
        if len(pieces) != 2 or not pieces[1].endswith("%"):
            return None
        colour = parse_color(pieces[0].strip())
        try:
            position = float(pieces[1][:-1])
        except ValueError:
            return None
        if colour is None:
            return None
        stops.append((colour, position))
    return stops if len(stops) >= 2 else None


def fmt_number(value: float) -> str:
    """A number the way both readers can parse it back exactly."""
    if value == int(value) and abs(value) < 1e15:
        return str(int(value))
    return f"{value:.4f}".rstrip("0").rstrip(".")


def encode_color(colour: tuple[int, int, int, float]) -> str:
    red, green, blue, alpha = colour
    if alpha >= 0.999:
        return f"#{red:02x}{green:02x}{blue:02x}"
    # Half up, spelled the way the channels above are (`int(n + 0.5)`), not
    # `round()`: the two readers have to land on the same byte, and Python's
    # round is half-to-even. It disagreed with the Rust reader on exactly one
    # token -- `rgba(27, 217, 106, 0.7)`, where 0.7 * 255 is 178.5 and the two
    # chose 178 and 179 -- which cost a night of chasing a copy that was right.
    byte = int(alpha * 255.0 + 0.5)
    return f"#{red:02x}{green:02x}{blue:02x}{byte:02x}"


def classify(text: str) -> tuple[str, str]:
    """(kind, encoded value) for one resolved value."""
    if text.startswith(("linear-gradient(", "radial-gradient(")):
        stops = parse_gradient(text)
        if stops is None:
            # A stop behind a var() -- --loading-bar-gradient is one -- has no
            # answer this reader can see, so it is kept as text and a claim
            # about it fails loudly rather than quietly matching half a value.
            return "Text", text
        encoded = ",".join(f"{encode_color(colour)}@{fmt_number(position)}" for colour, position in stops)
        return "Gradient", encoded
    colour = parse_color(text)
    if colour is not None:
        return "Color", encode_color(colour)
    for suffix in ("rem", "px"):
        if text.endswith(suffix):
            try:
                number = float(text[: -len(suffix)])
            except ValueError:
                break
            if suffix == "rem":
                number *= REM
            return "Length", fmt_number(number)
    try:
        return "Number", fmt_number(float(text))
    except ValueError:
        pass
    return "Text", text


def resolve(
    table: dict[str, tuple[str, int, str]], token: str, depth: int = 0
) -> tuple[str, str, int, str]:
    """(kind, encoded value, line, file) for one token, var() chains followed."""
    if depth > 8:
        raise SystemExit(f"--{token}: follows more than eight var() hops: that is a cycle")
    if token not in table:
        raise SystemExit(f"--{token} resolves to a token no block in these sheets declares")
    raw, line, file = table[token]
    value = cut_value(raw)
    found = BARE_VAR.match(value)
    if found:
        # The value is written where the chain ends, so the line reported is
        # the referent's -- the same rule the Rust reader applies.
        kind, encoded, referent, referent_file = resolve(table, found.group(1), depth + 1)
        return kind, encoded, referent, referent_file
    kind, encoded = classify(value)
    return kind, encoded, line, file


def rust_string(text: str) -> str:
    return '"' + text.replace("\\", "\\\\").replace('"', '\\"') + '"'


def parse_scoped(text: str) -> list[tuple[str, str, str, int]]:
    """(selector, token, raw value, line) for every declaration in a file.

    Not `parse_blocks`, which is built around the two global sheets: there a
    declaration sits directly under a top-level block. These files are SFC style
    blocks and nested rules, so what is reported is the *innermost rule* a
    declaration sits in -- at-rules are transparent, which is what makes
    `@media (prefers-reduced-motion) { .a { --x: 1 } }` report `.a` rather than
    the query, and a selector that runs over several lines is joined with single
    spaces so both readers derive the same string.
    """
    out: list[tuple[str, str, str, int]] = []
    stack: list[str] = []
    pending: tuple[str, str, str, int] | None = None

    for lineno, raw in enumerate(text.splitlines(), start=1):
        line = raw.strip()
        if not line:
            continue
        if line.startswith("<style") or line.startswith("</style"):
            # A component can carry more than one style block, and the rules of
            # one are not the rules of the next.
            stack.clear()
            pending = None
            continue
        if pending is not None:
            selector, name, value, at = pending
            joined = f"{value} {line}"
            if ";" in joined:
                out.append((selector, name, joined, at))
                pending = None
            else:
                pending = (selector, name, joined, at)
            continue
        if line.endswith("{"):
            stack.append(" ".join(line[:-1].split()))
            continue
        if line.startswith("}"):
            if stack:
                stack.pop()
            continue
        # A declaration starts with `--` *and* holds a colon. A continuation of
        # a property above it can start with `--` without being one --
        # ScrollablePanel.vue transitions `--_top-fade-height 0.05s linear,` on a
        # line of its own -- and reading that as a declaration invents a token,
        # which the copy would then hold as if the reference had declared it.
        if line.startswith("--") and ":" in line:
            name, _, value = line.partition(":")
            selector = next((part for part in reversed(stack) if not part.startswith("@")), "")
            name, value = name.strip(), value.strip()
            if ";" in value:
                out.append((selector, name, value, lineno))
            else:
                pending = (selector, name, value, lineno)
    return out


def scoped_rows() -> list[tuple[str, int, str, str, str, str]]:
    """(file, line, selector, token, kind, value) for every scoped declaration.

    The file is relative to the vendored tree's root with forward slashes, so
    the copy reads the same whichever machine generated it -- the Rust reader
    normalizes its own paths to match.
    """
    rows: list[tuple[str, int, str, str, str, str]] = []
    for path in sorted(TREE.rglob("*")):
        if not path.is_file() or path.suffix not in STYLE_SUFFIXES:
            continue
        if path.resolve() in GLOBAL_SHEETS or "node_modules" in path.parts:
            continue
        text = path.read_text(encoding="utf-8")
        for selector, name, raw, line in parse_scoped(text):
            kind, encoded = classify(cut_value(raw))
            rows.append((path.relative_to(TREE).as_posix(), line, selector, name, kind, encoded))
    return rows


def load_cascade() -> list[tuple[str, dict[str, tuple[str, str, int]]]]:
    """Every mode, resolved. The order of the modes is the order of the tables."""
    variable_blocks = {block.selector: block for block in parse_blocks(VARIABLES.read_text(encoding="utf-8"))}
    default_blocks = parse_blocks(DEFAULTS.read_text(encoding="utf-8"))

    def wanted(selector: str) -> Block:
        if selector not in variable_blocks:
            raise SystemExit(f"variables.scss no longer declares a `{selector}` block: update this generator")
        return variable_blocks[selector]

    light = wanted(".light-properties")
    html = wanted("html")
    dark = wanted(".dark-mode, .dark, :root[data-theme='dark']")
    oled = wanted(".oled-mode")
    retro = wanted(".retro-mode")
    bodies = [block for block in default_blocks if block.selector == "body"]
    if len(bodies) != 1:
        raise SystemExit(f"defaults.scss declares {len(bodies)} `body` blocks, expected exactly one")
    body = bodies[0]

    # The extends are the cascade; if upstream ever changes them, this file and
    # the Rust checker have to change with it, so fail here rather than guess.
    for block, expected in ((html, ".light-properties"), (oled, ".dark-mode"), (retro, ".dark-mode")):
        if block.parent != expected:
            raise SystemExit(
                f"`{block.selector}` now extends {block.parent!r}, not {expected!r}: "
                "the cascade this generator and the gate both build has moved"
            )

    # Every declaration carries the file its line belongs to, because a line
    # number alone cannot: `--font-size-xs` is line 21 of defaults.scss, and
    # the gate's failure message has to name the right one.
    def tagged(tokens: dict[str, tuple[str, int]], file: str) -> dict[str, tuple[str, int, str]]:
        return {name: (raw, line, file) for name, (raw, line) in tokens.items()}

    base = tagged(light.tokens, "variables.scss")
    base.update(tagged(html.tokens, "variables.scss"))
    base.update(tagged(body.tokens, "defaults.scss"))
    dark_tokens = tagged(dark.tokens, "variables.scss")
    modes: list[tuple[str, dict[str, tuple[str, int, str]]]] = [
        ("LIGHT", base),
        ("DARK", {**base, **dark_tokens}),
        ("OLED", {**base, **dark_tokens, **tagged(oled.tokens, "variables.scss")}),
        ("RETRO", {**base, **dark_tokens, **tagged(retro.tokens, "variables.scss")}),
    ]
    return [(label, {name: resolve(table, name) for name in sorted(table)}) for label, table in modes]


HEADER = """//! The reference's full token vocabulary, generated -- not written by hand.
//!
//! `python tools/gen_tokens.py` reads the vendored sheets at the commit
//! `UPSTREAM.md` pins ({commit}) and emits every `--token` the reference
//! declares, per mode, after the reference's own cascade and with `var()`
//! chains followed to the end. {counts}
//!
//! This file is the shell's *copy* of the vocabulary. Whether that copy is
//! still the sheets is not taken on trust: `the_generated_vocabulary_is_the_sheets`
//! re-reads the sheets itself and fails here, naming the file and line the
//! reference states the moved token on, when the two disagree. So the rule is:
//! never edit this file by hand -- change the sheet or the generator, run the
//! generator, commit what it writes.
//!
//! It holds two kinds of thing. The **mode tables** -- `LIGHT`, `DARK`, `OLED`,
//! `RETRO` -- are the global cascade: {counts}. `SCOPED` is the rest of the
//! vocabulary, {scoped}: the tokens a single stylesheet, component or page sets
//! for one selector (`--hover-brightness: 1` on a card, `--ease-out-expo` on
//! `:root`, the `--os-*` scrollbar knobs a combobox configures). Those have no
//! mode to resolve in and no cascade to merge into, so they are kept beside the
//! selector that sets them -- and they are in the copy for the same reason the
//! rest is: the reference states them, and a value in a comment cannot fail.
//!
//! Compiled into the test build only (`#[cfg(test)]` in `main.rs`): the
//! palette paints from `theme.rs`, and this copy is what the vocabulary gate
//! checks the sheets against. When a page starts consuming a token at
//! runtime, the token moves into `theme.rs` and this row stays the receipt.
//!
//! `Kind::Text` holds everything the reference declares that has no drawn
//! answer here (its `hsla()` shadows, its font stacks): the shell holds the
//! token, and a claim that needs the value fails loudly instead of quietly
//! matching nothing.

//! Generated from assets/styles/variables.scss and assets/styles/defaults.scss.

/// What kind of value a resolved token carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {{
    /// A colour, encoded `#rrggbb` or `#rrggbbaa` when the reference writes an
    /// alpha into it.
    Color,
    /// A length, in px, at the reference's 16px root.
    Length,
    /// A unitless number.
    Number,
    /// A gradient's stops, encoded `#rrggbbaa@position`, position in percent,
    /// comma-separated, in order.
    Gradient,
    /// Anything else the reference declares -- shadows, font stacks, blend
    /// modes -- kept as the reference's own text.
    Text,
}}

/// One token of the vocabulary, with the file and line it is declared on.
#[derive(Debug, Clone, Copy)]
pub struct Row {{
    pub token: &'static str,
    pub file: &'static str,
    pub line: u32,
    pub kind: Kind,
    pub value: &'static str,
}}

/// One token a single sheet, component or page declares for one selector.
///
/// Scoped rather than global: it applies to that rule, not to a mode. The
/// selector is the innermost *rule* the declaration sits in -- at-rules are
/// transparent, so a token set inside `@media` is reported against the rule the
/// query wraps.
#[derive(Debug, Clone, Copy)]
pub struct Scoped {{
    pub file: &'static str,
    pub line: u32,
    pub selector: &'static str,
    pub token: &'static str,
    pub kind: Kind,
    pub value: &'static str,
}}
"""


def emit(
    modes: list[tuple[str, dict[str, tuple[str, str, int, str]]]],
    scoped: list[tuple[str, int, str, str, str, str]],
) -> str:
    counts = ", ".join(f"{len(table)} {label.lower()}" for label, table in modes)
    files = len({row[0] for row in scoped})
    out = [
        HEADER.format(
            commit=upstream_commit(),
            counts=counts,
            scoped=f"{len(scoped)} declarations in {files} files, outside those two sheets",
        )
    ]
    for label, table in modes:
        out.append(f"pub const {label}: &[Row] = &[")
        for name, (kind, value, line, file) in table.items():
            out.append(
                f"    Row {{ token: {rust_string(name)}, file: {rust_string(file)}, "
                f"line: {line}, kind: Kind::{kind}, value: {rust_string(value)} }},"
            )
        out.append("];")
        out.append("")
    out.append("pub const SCOPED: &[Scoped] = &[")
    for file, line, selector, token, kind, value in scoped:
        out.append(
            f"    Scoped {{ file: {rust_string(file)}, line: {line}, "
            f"selector: {rust_string(selector)}, token: {rust_string(token)}, "
            f"kind: Kind::{kind}, value: {rust_string(value)} }},"
        )
    out.append("];")
    return "\n".join(out).rstrip("\n") + "\n"


def main() -> None:
    for path in (VARIABLES, DEFAULTS, UPSTREAM):
        if not path.exists():
            raise SystemExit(f"{path} is missing: the vendored reference tree is not checked out")
    modes = load_cascade()
    scoped = scoped_rows()
    # A table that came out empty or tiny means the parser stopped matching the
    # sheets' shape, which must fail here rather than ship as a short copy.
    for label, table in modes:
        if len(table) < 150:
            raise SystemExit(f"{label} resolved to {len(table)} tokens, far below what these sheets declare")
    if len(scoped) < 60:
        raise SystemExit(
            f"the scoped walk found {len(scoped)} declarations, and these files declare more: "
            "a parser that stopped matching the shape would ship a short copy silently"
        )
    written = emit(modes, scoped)
    OUT.write_text(written, encoding="utf-8", newline="\n")
    for label, table in modes:
        colours = sum(1 for _, (kind, _, _, _) in table.items() if kind == "Color")
        gradients = sum(1 for _, (kind, _, _, _) in table.items() if kind == "Gradient")
        lengths = sum(1 for _, (kind, _, _, _) in table.items() if kind == "Length")
        numbers = sum(1 for _, (kind, _, _, _) in table.items() if kind == "Number")
        texts = sum(1 for _, (kind, _, _, _) in table.items() if kind == "Text")
        print(
            f"{label}: {len(table)} tokens -- {colours} colours, {lengths} lengths, "
            f"{numbers} numbers, {gradients} gradients, {texts} text"
        )
    print(
        f"scoped: {len(scoped)} declarations in {len({row[0] for row in scoped})} file(s), "
        "outside the two global sheets"
    )
    print(f"wrote {OUT.relative_to(REPO)} ({len(written.splitlines())} lines)")


if __name__ == "__main__":
    sys.exit(main())
