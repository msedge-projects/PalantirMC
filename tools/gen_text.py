#!/usr/bin/env python3
"""Compile the reference client's own strings into a Rust table.

The source is the reference's English locale, in the two files its own i18n
config merges (`app-frontend/src/i18n.config.ts`):

    app-frontend/src/locales/en-US/index.json   1516 leaves, `{message}` each
    ui/src/locales/en-US/index.json             2330 leaves, `{defaultMessage}` each

They do not overlap (a collision is an error here), and between them they are the
whole of the reference's English copy. Compiling them is the same argument as
`gen_theme.py` and `gen_icons.py`: a string transcribed by hand is a string that
drifts, and the reference is the only authority on what its own interface says.

    python tools/gen_text.py            # write crates/palantir-desktop/src/text_gen.rs
    python tools/gen_text.py --check    # fail if that file is not what this emits
    python tools/gen_text.py --report   # what is in the locale, and what is refused

## The ICU subset, and why refusing is the point

353 of the 3846 strings are not plain text: they carry ICU MessageFormat. The
reference hands them to vue-i18n, which compiles ICU with
`@intlify/message-compiler`, so a faithful port has to understand exactly the
constructs its copy uses. Those are, counted by this tool:

    {name}                                  a value, rendered as it was given
    {name, number}                          a grouped integer
    {name, plural, one {..} other {..}}     an English plural, `#` for the value
    {name, select, key {..} other {..}}     a choice between whole alternatives

and nothing else. `date`, `time`, `list`, `duration`, `selectordinal`, a plural
category English has no rule for, and ICU's single-apostrophe quoting are *not* in
the copy, and this tool stops with the key's name if one ever appears rather than
emitting a helper that renders it wrongly. A vendored update that introduces one
is a failure here, which is the visible outcome; a silently wrong string is not.

Two details of the compiler's behaviour are reproduced rather than approximated,
because the reference's copy depends on both:

* **`{count}` and `{count, number}` render differently.** A *typed* argument is
  formatted by `Intl.NumberFormat`, so 1200 becomes `1,200`; a bare `{count}` is
  interpolated as the value itself, so the same 1200 stays `1200`. `Plural` carries
  both renderings, and the generated helper asks for the one the message declared.
* **A plural argument may be a number or a category string.** The reference passes
  both: `{count, plural, one {# project} other {# projects}}` is called with a
  count, while `{countPlural, plural, one {player} other {players}}` is called with
  `formatCompactNumberPlural(...)` -- a string like `1.2K`, chosen for that message
  precisely. [`crate::text::Plural`] holds both, so neither caller has to be guessed
  at, and `#` renders as the number or as the string it was handed.

## What a generated name is

The Rust name is the reference's key: `.` and `-` both mark a word boundary, so
`app.action-bar.downloading-java` is `AppActionBarDownloadingJava` and, for the
helpers, `app_action_bar_downloading_java`. The key itself is kept verbatim as the
value of `name()`, so a generated name can always be traced back to the string it
came from -- and `--check` on the keys is what makes a renamed upstream key visible
instead of silently unmatched.
"""

from __future__ import annotations

import argparse
import json
import pathlib
import re
import sys
import unicodedata
from dataclasses import dataclass, field

ROOT = pathlib.Path(__file__).resolve().parent.parent
SOURCES = [
    pathlib.Path("vendor/modrinth-app/app-frontend/src/locales/en-US/index.json"),
    pathlib.Path("vendor/modrinth-app/ui/src/locales/en-US/index.json"),
]
OUTPUT = pathlib.Path("crates/palantir-desktop/src/text_gen.rs")

# The plural categories English has a rule for. Anything else is refused by
# default, which is what keeps this tool's English output the same as it was
# before there was a `tools/gen_locale.py`: that file is checked against this
# one, so widening the default would silently change 3846 generated helpers.
#
# The whole CLDR category set is named beside it because the other 32 locales
# need it -- `zero`, `two`, `few` and `many` are real arms in `ar-SA`, `pl-PL`,
# `ru-RU`, `cs-CZ` and `sr-CS` -- and a locale is compiled with that language's
# own set rather than English's. See `tools/gen_locale.py`.
PLURAL_CATEGORIES = {"one", "other"}
ALL_PLURAL_CATEGORIES = {"zero", "one", "two", "few", "many", "other"}

RUST_KEYWORDS = {
    "as", "async", "await", "box", "break", "const", "continue", "crate", "dyn",
    "else", "enum", "extern", "false", "fn", "for", "if", "impl", "in", "let",
    "loop", "match", "mod", "move", "mut", "pub", "ref", "return", "self", "static",
    "struct", "super", "trait", "true", "type", "unsafe", "use", "where", "while",
    "yield",
}


class Refused(Exception):
    """A construct the reference's copy does not use, met while compiling."""


# ---- Reading the locale --------------------------------------------------


def leaves(node, prefix=""):
    """Every message under `node`, as (key, text)."""
    if isinstance(node, dict):
        # A leaf is the reference's own shape: one `message` (app) or one
        # `defaultMessage` (ui), and nothing else.
        if node and set(node) <= {"message", "defaultMessage"}:
            text = node.get("message")
            if text is None:
                text = node["defaultMessage"]
            if not isinstance(text, str):
                raise Refused(f"{prefix}: a message that is not a string")
            yield prefix, text
            return
        for key, value in node.items():
            yield from leaves(value, f"{prefix}.{key}" if prefix else key)
        return
    raise Refused(f"{prefix}: a value that is not a message object")


def load(path: pathlib.Path) -> dict:
    document = json.loads((ROOT / path).read_text(encoding="utf-8"))
    return dict(leaves(document))


def load_all() -> dict:
    """Both files, merged the way the reference merges them.

    The reference spreads the two objects into one message bag and does not
    expect a key to be in both. This refuses a collision rather than letting the
    later file quietly win: a collision would mean the reference renders one of
    the two strings and never the other, which is a fact about upstream and not
    something to paper over here.
    """
    messages: dict = {}
    for path in SOURCES:
        for key, text in load(path).items():
            if key in messages:
                raise Refused(f"{key}: defined in more than one locale file")
            messages[key] = text
    return messages


# ---- ICU -----------------------------------------------------------------


@dataclass(frozen=True)
class Lit:
    """Literal text, already unescaped."""

    text: str


@dataclass(frozen=True)
class Arg:
    """`{name}`: a value, rendered as given."""

    name: str


@dataclass(frozen=True)
class Number:
    """`{name, number}`: a grouped integer."""

    name: str


@dataclass(frozen=True)
class Hash:
    """`#` inside a plural arm: the plural's own value."""

    name: str


@dataclass(frozen=True)
class Plural:
    """`{name, plural, ...}`."""

    name: str
    arms: tuple


@dataclass(frozen=True)
class Select:
    """`{name, select, ...}`."""

    name: str
    arms: tuple


@dataclass
class Counts:
    """What the walk met: node counts for the report, message counts for the gate."""

    nodes: dict = field(default_factory=lambda: {"plural": 0, "select": 0, "number": 0})
    hashes: int = 0

    def node(self, kind: str) -> None:
        self.nodes[kind] = self.nodes.get(kind, 0) + 1


def read_braces(text: str, start: int) -> tuple:
    """The contents of the `{...}` opening at `start`, and the index after it."""
    depth = 0
    for index in range(start, len(text)):
        if text[index] == "{":
            depth += 1
        elif text[index] == "}":
            depth -= 1
            if depth == 0:
                return text[start + 1 : index], index + 1
    raise Refused(f"unbalanced braces in {text!r}")


def split_top_level(text: str, on: str = ",") -> list:
    """Split on a separator that is not inside braces."""
    parts, depth, current = [], 0, ""
    for character in text:
        if character == "{":
            depth += 1
        elif character == "}":
            depth -= 1
        if character == on and depth == 0:
            parts.append(current)
            current = ""
        else:
            current += character
    parts.append(current)
    return parts


def parse_arms(body: str, in_plural: bool, counts: Counts, allowed=PLURAL_CATEGORIES) -> tuple:
    """Read `key {body} key {body} ...`, in the order the reference writes them.

    A *plural*'s arms are the categories, so anything but `one`, `other` and `=N`
    is refused: English has no rule for `few`, and inventing one would be a wrong
    string rather than a visible failure. A *select*'s arms are names the caller
    chose (`fiveDays`, `monthly`, `downgrade`), so any key is a key.
    """
    arms: list = []
    index, length = 0, len(body)
    while index < length:
        while index < length and body[index].isspace():
            index += 1
        if index >= length:
            break
        brace = body.find("{", index)
        if brace < 0:
            raise Refused(f"an arm with no body in {body!r}")
        key = body[index:brace].strip()
        if not key:
            raise Refused(f"an arm with no key in {body!r}")
        if in_plural and key != "other" and not key.startswith("=") and key not in allowed:
            raise Refused(f"the plural category {key!r}")
        inner, index = read_braces(body, brace)
        arms.append((key, tuple(parse_nodes(inner, in_plural, counts, allowed))))
    if not arms or arms[-1][0] != "other":
        raise Refused(f"an arm list without an `other` in {body!r}")
    return tuple(arms)


def parse_placeholder(inner: str, in_plural: bool, counts: Counts, allowed=PLURAL_CATEGORIES):
    parts = [part.strip() for part in split_top_level(inner)]
    name = parts[0]
    if not re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*", name):
        raise Refused(f"the argument name {name!r}")
    kind = parts[1] if len(parts) > 1 else ""
    if not kind:
        return Arg(name)
    if kind == "number":
        counts.node("number")
        return Number(name)
    if kind in {"plural", "selectordinal", "select"}:
        if kind == "selectordinal":
            raise Refused("`selectordinal`")
        counts.node(kind)
        body = ",".join(parts[2:]) if len(parts) > 2 else ""
        arms = parse_arms(body, in_plural=kind == "plural", counts=counts, allowed=allowed)
        return Plural(name, arms) if kind == "plural" else Select(name, arms)
    raise Refused(f"the ICU type {kind!r}")


def parse_nodes(message: str, in_plural: bool, counts: Counts, allowed=PLURAL_CATEGORIES) -> list:
    """Parse a message (or an arm's body) into nodes."""
    nodes: list = []
    index, length = 0, len(message)
    literal = ""

    def flush() -> None:
        nonlocal literal
        if literal:
            nodes.append(Lit(literal))
            literal = ""

    while index < length:
        character = message[index]
        if character == "'":
            # ICU quoting: `''` is one literal apostrophe. A single apostrophe
            # before a brace would open a quoted run, which the reference never
            # writes, so it is refused rather than half-implemented.
            if message.startswith("''", index):
                literal += "'"
                index += 2
                continue
            if index + 1 < length and message[index + 1] == "{":
                raise Refused(f"ICU quoting in {message!r}")
            literal += "'"
            index += 1
            continue
        if character == "#" and in_plural:
            flush()
            counts.hashes += 1
            nodes.append(Hash(""))
            index += 1
            continue
        if character == "{":
            flush()
            inner, index = read_braces(message, index)
            nodes.append(parse_placeholder(inner, in_plural, counts, allowed))
            continue
        literal += character
        index += 1
    flush()
    return nodes


def annotate_hashes(nodes: list, plural: str) -> list:
    """Fill in the argument each `#` stands for: the arm's own plural."""
    result = []
    for node in nodes:
        if isinstance(node, Hash):
            result.append(Hash(plural))
        elif isinstance(node, (Plural, Select)):
            result.append(
                type(node)(
                    node.name,
                    tuple((key, tuple(annotate_hashes(body, node.name))) for key, body in node.arms),
                )
            )
        else:
            result.append(node)
    return result


def parse_message(counts: Counts, template: str, allowed=PLURAL_CATEGORIES) -> list:
    return annotate_hashes(parse_nodes(template, False, counts, allowed), "")


# ---- Naming --------------------------------------------------------------


def words(key: str) -> list:
    return [word for word in re.split(r"[.\-_]+", key) if word]


def sanitize(name: str) -> str:
    """Everything a Rust identifier cannot hold is dropped.

    The locale has one key with a `+` in it (`tag.category.512x+`), and a
    generated name that will not compile is a missing string rather than a
    visible failure, so the characters go rather than the name being refused. A
    collision that dropping them causes is caught below, by name.
    """
    name = re.sub(r"[^A-Za-z0-9_]", "", name)
    return f"Key{name}" if not name or name[0].isdigit() else name


def type_name(key: str) -> str:
    return sanitize("".join(word[:1].upper() + word[1:] for word in words(key)))


def function_name(key: str) -> str:
    return sanitize("_".join(word.lower() for word in words(key)))


def argument(name: str) -> str:
    """`contentType` as Rust spells it: the reference's name, snake-cased."""
    snake = re.sub(r"(?<!^)(?=[A-Z])", "_", name).lower()
    return f"{snake}_arg" if snake in RUST_KEYWORDS else snake


def escape(text: str) -> str:
    r"""A Rust string literal holding `text`.

    Characters in the Unicode *format* and *control* categories are written as
    `\u{...}` escapes even though Rust would accept them raw. English's own
    locale has none, so this changes nothing about `text_gen.rs` -- but the
    reference's translations do, and every one of them is invisible: `pt-BR`
    writes `usados \u{200b}\u{200b}apenas` and `ru-RU` writes `\u{200b}\u{200b}`
    inside a sentence, which is a Crowdin artifact rather than a word break.
    Emitted raw they make the source of `locale_gen.rs` contain characters no
    editor shows, and `clippy::invisible_characters` -- a *correctness* lint, so
    `-D clippy::correctness` denies it -- fails the build over them.

    The escape is a faithful round trip: `\u{200b}` decodes to the same string
    the locale holds. Stripping the character instead would make the table agree
    with clippy rather than with the reference, which is the wrong way round.
    """
    out = []
    for character in text:
        if character == "\\":
            out.append("\\\\")
        elif character == '"':
            out.append('\\"')
        elif character == "\n":
            out.append("\\n")
        elif character == "\r":
            out.append("\\r")
        elif character == "\t":
            out.append("\\t")
        elif unicodedata.category(character) in {"Cf", "Cc", "Zl", "Zp"}:
            out.append(f"\\u{{{ord(character):X}}}")
        else:
            out.append(character)
    return '"' + "".join(out) + '"'


def rust_char(char: str) -> str:
    """A Rust character literal holding `char`."""
    if char == "'":
        return "'\\''"
    if char == "\\":
        return "'\\\\'"
    if char == "\n":
        return "'\\n'"
    if char == "\r":
        return "'\\r'"
    if char == "\t":
        return "'\\t'"
    return f"'{char}'"


def emit_text(text: str) -> str:
    """The statement that appends `text` to `out`.

    A one-character literal is a `push`, not a `push_str`. The reference has 53
    of them -- separators, a full stop, a parenthesis -- and this generator's
    whole point is that the shell carries no warning it did not choose: the fix
    belongs here, where the code is written once, rather than in the 53 places a
    hand-written file would have made.
    """
    if len(text) == 1:
        return f"out.push({rust_char(text)})"
    return f"out.push_str({escape(text)})"


# ---- How an argument is used --------------------------------------------


@dataclass
class Uses:
    """How one argument of one message is used, which decides its type."""

    bare: bool = False
    number: bool = False
    plural: bool = False
    select: bool = False

    @property
    def kind(self) -> str:
        if self.select and (self.plural or self.number):
            raise Refused("an argument used as both a select key and a plural or a number")
        if self.select:
            return "select"
        if self.plural:
            return "plural"
        if self.number:
            return "number"
        return "bare"


def collect(nodes, into: dict) -> None:
    for node in nodes:
        if isinstance(node, Arg):
            into.setdefault(node.name, Uses()).bare = True
        elif isinstance(node, Number):
            into.setdefault(node.name, Uses()).number = True
        elif isinstance(node, Plural):
            into.setdefault(node.name, Uses()).plural = True
            for _, body in node.arms:
                collect(body, into)
        elif isinstance(node, Select):
            into.setdefault(node.name, Uses()).select = True
            for _, body in node.arms:
                collect(body, into)


def rust_type(name: str, uses: Uses) -> str:
    kind = uses.kind
    if kind == "select":
        return "&str"
    if kind == "plural":
        return f"impl Into<text::Plural<{lifetime(name)}>>"
    if kind == "number":
        return "u64"
    return "&str"


def lifetime(name: str) -> str:
    """A lifetime per plural argument, named after it.

    Two plural arguments of one message may come from different strings, so they
    cannot share one lifetime.
    """
    return f"'{argument(name)}"


# ---- Emitting the body ---------------------------------------------------


def emit_nodes(nodes, uses: dict, out: list, indent: str) -> None:
    """Emit the statements that append `nodes` to `out`."""
    for node in nodes:
        if isinstance(node, Lit):
            out.append(f"{indent}{emit_text(node.text)};")
        elif isinstance(node, Arg):
            binding = argument(node.name)
            if uses[node.name].kind == "plural":
                out.append(f"{indent}out.push_str(&{binding}.bare());")
            else:
                out.append(f"{indent}out.push_str({binding});")
        elif isinstance(node, Number):
            binding = argument(node.name)
            if uses[node.name].kind == "plural":
                out.append(f"{indent}out.push_str(&{binding}.grouped());")
            else:
                out.append(f"{indent}out.push_str(&text::number({binding}));")
        elif isinstance(node, Hash):
            # `#` in a plural arm is that arm's own value: the grouped number when
            # it is a number, and the string it was handed when it is a category.
            out.append(f"{indent}out.push_str(&{argument(node.name)}.grouped());")
        elif isinstance(node, Plural):
            binding = argument(node.name)
            # `=N` arms are exact matches and are tried first, then `one`, then
            # `other`; ICU's own order, and the reason the reference's copy reads
            # the way it does.
            arms = sorted(
                node.arms,
                key=lambda arm: (2 if arm[0] == "other" else 0 if arm[0].startswith("=") else 1),
            )
            for index, (key, body) in enumerate(arms):
                if index == 0:
                    head = f"if {binding}.is({escape(key)}) {{" if key != "other" else "{"
                    out.append(f"{indent}{head}")
                else:
                    # Attached to the previous arm's closing brace, so the chain
                    # reads `} else if ... {` the way it would be written by hand.
                    head = f"else if {binding}.is({escape(key)}) {{" if key != "other" else "else {"
                    out[-1] = f"{out[-1]} {head}"
                emit_nodes(body, uses, out, indent + "    ")
                out.append(f"{indent}}}")
        elif isinstance(node, Select):
            binding = argument(node.name)
            out.append(f"{indent}match {binding} {{")
            for key, body in node.arms:
                out.append(f"{indent}    {'_' if key == 'other' else escape(key)} => {{")
                emit_nodes(body, uses, out, indent + "        ")
                out.append(f"{indent}    }}")
            out.append(f"{indent}}}")
        else:  # pragma: no cover - the parser only builds the nodes above
            raise Refused(f"a node this tool does not know: {node!r}")


def emit_helper(key: str, template: str, nodes: list, counts: Counts) -> str:
    """One generated function, for one message with ICU markers in it."""
    uses: dict = {}
    collect(nodes, uses)
    parameters = ", ".join(f"{argument(name)}: {rust_type(name, p)}" for name, p in uses.items())
    lifetimes = [lifetime(name) for name, p in uses.items() if p.kind == "plural"]
    generics = f"<{', '.join(lifetimes)}>" if lifetimes else ""
    lines = [
        f"/// `{key}`",
        "///",
        "/// ```text",
        f"/// {template}",
        "/// ```",
        "///",
        "/// The language in force is read first, when its table carries this key and its",
        "/// template is one [`crate::text`] can fill in. What follows is English, which is",
        "/// also what runs for English itself.",
        f"pub fn {function_name(key)}{generics}({parameters}) -> String {{",
    ]
    for name, p in uses.items():
        if p.kind == "plural":
            binding = argument(name)
            lines.append(f"    let {binding} = {binding}.into();")
    # The locale's own sentence, before the English one. Each argument is handed
    # over by *kind* rather than as a string, because a locale may use it
    # differently from English -- `pt-BR` writes `{count, number}` where English
    # writes `{count}` -- and the renderer has to know which it is.
    arguments = []
    for name, p in uses.items():
        binding = argument(name)
        if p.kind == "plural":
            arguments.append(f'("{name}", text::Value::plural({binding}))')
        elif p.kind == "number":
            arguments.append(f'("{name}", text::Value::number({binding}))')
        else:
            arguments.append(f'("{name}", text::Value::text({binding}))')
    if arguments:
        lines.append("    if let Some(localized) = text::render(")
        lines.append(f"        Key::{type_name(key)},")
        lines.append(f"        &[{', '.join(arguments)}],")
        lines.append("    ) {")
        lines.append("        return localized;")
        lines.append("    }")
    lines.append("    let mut out = String::new();")
    emit_nodes(nodes, uses, lines, "    ")
    lines.append("    out")
    lines.append("}")
    return "\n".join(lines)


# ---- Quoted-from-the-locale samples --------------------------------------

# Messages whose text is quoted straight into a generated test, so that a
# generator that lost its way cannot agree with itself in silence.
SAMPLE_KEYS = [
    "app.action-bar.downloads",
    "app.settings.tabs.appearance",
    "settings.display.theme.dark",
    "settings.display.theme.retro",
    "settings.display.theme.system",
    "ui.stacked-admonitions.alert-count",
]

# One call per shape the generated helpers have: a bare argument, a typed number,
# a plural with `#`, a plural whose value is a category string, `=0`, a select, a
# plural nested inside a select, and an argument used both bare and as a plural.
SAMPLE_CALLS = [
    "app_action_bar_downloading_java(\"21\")",
    "app_screenshots_selection_delete_description(1u64)",
    "app_screenshots_selection_delete_description(3u64)",
    "app_settings_synced_options_multiplayer_servers_search(4u64)",
    "settings_language_languages_search_results_announcement(0u64)",
    "settings_language_languages_search_results_announcement(2u64)",
    "servers_listing_notice_pending_change(\"downgrade\", \"2GB\", \"1 January\")",
    "time_frame_picker_last_timeframe(3u64, \"hours\")",
    "format_bytes_0(1u64)",
    "project_recent_plays(\"25K\", \"other\")",
    "project_online_player_count_tooltip(\"1.2K\", \"one\")",
    "project_server_ping_ms(1234u64)",
]

# The plural behaviour, asserted arithmetically rather than against a constant.
SAMPLE_PLURAL = "browse.selected-projects-floating-bar.selected-count"
SAMPLE_CATEGORY = "project.online-player-count.tooltip"
SAMPLE_NUMBER = "project.server.ping.ms"


def emit(messages: dict, counts: Counts) -> str:
    keys = sorted(messages)
    names = [type_name(key) for key in keys]
    duplicates = sorted({name for name in names if names.count(name) > 1})
    if duplicates:
        raise Refused(f"key names that collide: {duplicates[:5]}")

    parsed = {key: parse_message(counts, messages[key]) for key in keys}
    helpers = [key for key in keys if "{" in messages[key]]

    # The coarse counts the generated test re-derives from the table itself: a
    # second reading, with a predicate that shares nothing with the parser above.
    coarse = {
        kind: sum(1 for key in keys if f", {kind}" in messages[key])
        for kind in ("plural", "select", "number")
    }
    coarse["#"] = sum(
        1 for key in keys if ", plural," in messages[key] and "#" in messages[key]
    )

    lines: list = []
    lines.append("//! The reference client's own strings, compiled from its vendored English")
    lines.append("//! locale.")
    lines.append("//!")
    lines.append("//! Generated by `tools/gen_text.py` -- do not edit by hand. Run")
    lines.append("//! `python tools/gen_text.py` to regenerate, and `--check` to see whether this")
    lines.append("//! file is what the tool emits. CI runs the check, so a vendored locale and this")
    lines.append("//! table cannot drift apart.")
    lines.append("//!")
    lines.append("//! Every string the interface shows comes from here. That is the point: the")
    lines.append("//! reference is the authority on its own copy, and a sentence retyped by hand")
    lines.append("//! is a sentence that drifts. [`Key::name`] is the reference's key verbatim, so")
    lines.append("//! a generated name can always be traced back to the file it came from.")
    lines.append("//!")
    lines.append("//! The strings that carry ICU MessageFormat are compiled into the helper")
    lines.append("//! functions below, with the reference's own argument names as their parameters.")
    lines.append("//! [`crate::text`] is the runtime they are built from, and says what is")
    lines.append("//! deliberately refused rather than approximated.")
    lines.append("//!")
    lines.append(f"//! {len(keys)} messages: {len(helpers)} of them carry ICU markers. Between them they")
    lines.append(
        f"//! use {counts.nodes['plural']} plurals, {counts.nodes['select']} selects and"
        f" {counts.nodes['number']} grouped"
    )
    lines.append(f"//! numbers, with {counts.hashes} `#` markers inside plural arms.")
    lines.append("")
    lines.append("#![allow(dead_code)]")
    lines.append("")
    lines.append("use crate::text;")
    lines.append("")
    lines.append("/// Every message the reference publishes, by generated name.")
    lines.append("///")
    lines.append("/// The order is the reference's keys in sort order, which is also the order of")
    lines.append("/// [`NAMES`] and [`MESSAGES`]: the three arrays are parallel, and a test says so.")
    lines.append("/// The reference's own keys include ones that end in `key`")
    lines.append("/// (`app.settings.game-options.keybind.key.keypad-key`), which is what")
    lines.append("/// `enum_variant_names` objects to. The names are the reference's, so the")
    lines.append("/// lint is allowed here rather than obeyed.")
    lines.append("#[allow(clippy::enum_variant_names)]")
    lines.append("#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]")
    lines.append("pub enum Key {")
    for key, name in zip(keys, names):
        lines.append(f"    /// `{key}`")
        lines.append(f"    {name},")
    lines.append("}")
    lines.append("")
    # `static`, not `const`: a 3846-entry const is inlined at every use, and
    # `large_const_arrays` is right to say so. The cost is that a `const fn`
    # cannot read a static, so the two accessors below are ordinary functions --
    # neither is called from a constant expression anywhere.
    lines.append("/// Every variant, in the same order as [`NAMES`] and [`MESSAGES`].")
    lines.append(f"pub static ALL: [Key; {len(keys)}] = [")
    lines.extend(f"    Key::{name}," for name in names)
    lines.append("];")
    lines.append("")
    lines.append("/// The reference's own key for each variant, in [`ALL`] order.")
    lines.append(f"pub static NAMES: [&str; {len(keys)}] = [")
    lines.extend(f"    {escape(key)}," for key in keys)
    lines.append("];")
    lines.append("")
    lines.append("/// The template for each variant, in [`ALL`] order, exactly as the locale")
    lines.append("/// writes it: ICU markers included, for the strings that have them.")
    lines.append(f"pub static MESSAGES: [&str; {len(keys)}] = [")
    lines.extend(f"    {escape(messages[key])}," for key in keys)
    lines.append("];")
    lines.append("")
    lines.append("impl Key {")
    lines.append("    /// The reference's own key, e.g. `app.action-bar.downloads`.")
    lines.append("    pub fn name(self) -> &'static str {")
    lines.append("        NAMES[self as usize]")
    lines.append("    }")
    lines.append("")
    lines.append("    /// The message as the locale writes it.")
    lines.append("    ///")
    lines.append("    /// For a string with ICU markers this is the *template*, which is what the")
    lines.append("    /// locale holds and what the helper beside this table fills in. Both are")
    lines.append("    /// generated from the same leaf, and a test asserts they are.")
    lines.append("    ///")
    lines.append("    /// The language in force wins when it has this key, which is the setting")
    lines.append("    /// rather than the table: `tools/gen_locale.py` compiles the other locales")
    lines.append("    /// and [`crate::locale`] decides which one is read. English is what this")
    lines.append("    /// returns for English itself and for any key a locale falls back on, so a")
    lines.append("    /// caller that draws a label needs no second code path.")
    lines.append("    pub fn message(self) -> &'static str {")
    lines.append("        let index = self as usize;")
    lines.append("        crate::locale::translated(index).unwrap_or(MESSAGES[index])")
    lines.append("    }")
    lines.append("}")
    lines.append("")
    lines.append("/// The variant a reference key names, if the locale has one.")
    lines.append("///")
    lines.append("/// A scan rather than a binary search over a sorted index: the lookup happens")
    lines.append("/// once per key at most, and a second sorted table would be another copy of the")
    lines.append("/// same 3846 strings in the binary.")
    lines.append("pub fn from_name(name: &str) -> Option<Key> {")
    lines.append("    ALL.into_iter().find(|key| key.name() == name)")
    lines.append("}")
    lines.append("")
    for key in helpers:
        lines.append(emit_helper(key, messages[key], parsed[key], counts))
        lines.append("")

    lines.extend(
        [
            "#[cfg(test)]",
            "mod tests {",
            "    use super::*;",
            "",
            "    #[test]",
            "    fn every_variant_names_itself_uniquely() {",
            "        let mut names: Vec<&str> = ALL.iter().map(|key| key.name()).collect();",
            f"        assert_eq!(names.len(), {len(keys)});",
            "        names.sort_unstable();",
            "        let count = names.len();",
            "        names.dedup();",
            "        assert_eq!(count, names.len(), \"two variants share a key\");",
            "        assert!(names.iter().all(|name| !name.is_empty()));",
            "    }",
            "",
            "    #[test]",
            "    fn a_key_can_be_read_back_out_of_its_name() {",
            "        for key in ALL {",
            "            assert_eq!(from_name(key.name()), Some(key));",
            "            assert_eq!(NAMES[key as usize], key.name());",
            "            assert_eq!(MESSAGES[key as usize], key.message());",
            "        }",
            "        assert_eq!(from_name(\"app.not.a.key\"), None);",
            "    }",
            "",
            "    #[test]",
            "    fn the_keys_are_in_order_so_a_scan_is_a_search() {",
            "        for pair in ALL.windows(2) {",
            "            assert!(pair[0].name() < pair[1].name(), \"{} is out of order\", pair[1].name());",
            "        }",
            "    }",
            "",
            "    #[test]",
            "    fn some_of_the_reference_s_own_strings_are_exactly_its_own() {",
            "        // Quoted from the locale this table was compiled from, so that a generator",
            "        // that lost its way cannot agree with itself in silence.",
        ]
    )
    for key in SAMPLE_KEYS:
        lines.append(f"        assert_eq!(Key::{type_name(key)}.message(), {escape(messages[key])});")
    lines.extend(
        [
            "    }",
            "",
            "    #[test]",
            "    fn the_icu_constructs_are_the_ones_the_reference_uses() {",
            "        // A second reading of the table, with a predicate that shares nothing with",
            "        // the generator's parser: a locale that starts using `date`, `time`, `list`",
            "        // or a plural category English has no rule for fails the tool, and a locale",
            "        // that quietly stops carrying what this table compiles fails here.",
        ]
    )
    for marker in ("plural", "select", "number", "#"):
        lines.append(f'        assert_eq!(count_occurrences("{marker}"), {coarse[marker]});')
    lines.extend(
        [
            "    }",
            "",
            "    /// How many messages carry one ICU construct, by looking for its marker.",
            "    fn count_occurrences(marker: &str) -> usize {",
            "        MESSAGES",
            "            .iter()",
            "            .filter(|message| {",
            "                if marker == \"#\" {",
            "                    message.contains(\", plural,\") && message.contains('#')",
            "                } else {",
            "                    message.contains(&format!(\", {marker}\"))",
            "                }",
            "            })",
            "            .count()",
            "    }",
            "",
            "    #[test]",
            "    fn a_plural_picks_its_arm_by_the_english_rule() {",
            f"        // Quoted from the reference: `{messages[SAMPLE_PLURAL]}`.",
            f"        assert_eq!({function_name(SAMPLE_PLURAL)}(1u64), {escape('1 project selected')});",
            f"        assert_eq!({function_name(SAMPLE_PLURAL)}(0u64), {escape('0 projects selected')});",
            f"        assert_eq!({function_name(SAMPLE_PLURAL)}(2500u64), {escape('2,500 projects selected')});",
            "        // And a category handed in as a string, which is what the reference does for",
            "        // a compacted count: `countPlural: formatCompactNumberPlural(online)`.",
            f"        // Quoted from: `{messages[SAMPLE_CATEGORY]}`.",
            f"        assert_eq!({function_name(SAMPLE_CATEGORY)}(\"1.2K\", \"one\"), {escape('1.2K player online')});",
            f"        assert_eq!({function_name(SAMPLE_CATEGORY)}(\"1.2K\", \"other\"), {escape('1.2K players online')});",
            "    }",
            "",
            "    #[test]",
            "    fn a_number_is_grouped_the_way_the_intl_formatter_groups_it() {",
            "        // `{count, number}` goes through `Intl.NumberFormat`; a bare `{count}`",
            "        // does not, and the difference is why `Plural` carries both renderings.",
            f"        // Quoted from: `{messages[SAMPLE_NUMBER]}`.",
            f"        assert_eq!({function_name(SAMPLE_NUMBER)}(12u64), {escape('12 ms')});",
            f"        assert_eq!({function_name(SAMPLE_NUMBER)}(1234u64), {escape('1,234 ms')});",
            "    }",
            "",
            "    #[test]",
            "    fn a_filled_message_has_no_icu_left_in_it() {",
        ]
    )
    for call in SAMPLE_CALLS:
        lines.append(f"        let filled = {call};")
        lines.append("        assert!(!filled.contains('{') && !filled.contains('}'), \"{filled}\");")
    lines.extend(["    }", "}", ""])
    return "\n".join(lines)


def main() -> int:
    parser = argparse.ArgumentParser(description="Compile the reference's strings.")
    parser.add_argument("--check", action="store_true", help="fail if the output is stale")
    parser.add_argument("--report", action="store_true", help="print what was found")
    arguments = parser.parse_args()

    counts = Counts()
    try:
        messages = load_all()
        generated = emit(messages, counts)
    except Refused as refusal:
        print(f"gen_text: refused {refusal}", file=sys.stderr)
        return 1

    if arguments.report:
        print(f"messages          {len(messages)}")
        print(f"with ICU markers  {sum(1 for text in messages.values() if '{' in text)}")
        for kind, count in sorted(counts.nodes.items()):
            print(f"{kind + ' nodes':<17} {count}")
        print(f"# markers         {counts.hashes}")

    path = ROOT / OUTPUT
    if arguments.check:
        current = path.read_text(encoding="utf-8") if path.exists() else ""
        if current != generated:
            print(
                f"gen_text: {OUTPUT} is not what this tool emits; run `python tools/gen_text.py`",
                file=sys.stderr,
            )
            return 1
        print("text generation is byte-identical")
        return 0

    path.write_text(generated, encoding="utf-8", newline="\n")
    print(f"gen_text: wrote {OUTPUT} ({len(generated.splitlines())} lines)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
