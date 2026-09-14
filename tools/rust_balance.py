#!/usr/bin/env python3
"""Structural check on hand-edited Rust, without a compiler.

Why this exists: this project's builds run on GitHub Actions, because a local
`cargo` run on the machine this is developed on costs a whole core for minutes.
That makes the compiler *remote*, and a remote compiler is a two-minute round
trip per typo. This is the cheap local filter that runs first: it parses nothing,
but it catches the one class of mistake a large hand-written edit makes most
often and a reviewer reads past — a brace, paren or bracket that does not close,
or a stray delimiter inside what only *looks* like code.

It deliberately does not try to be a parser. It knows four things:

* `//` to end of line is a comment (but not inside a string);
* `/* ... */` nests, as Rust's does;
* `"..."` with `\\` escapes, and raw strings `r"..."`, `r#"..."#`, `r##"..."##`;
* character literals `'x'` and lifetimes `'a`, which are told apart by where the
  next `'` is.

Anything it cannot be sure about it reports rather than guesses, so a clean run
means "nothing structural is wrong", not "this compiles".

    python tools/rust_balance.py crates/palantir-desktop/src/*.rs
"""

from __future__ import annotations

import sys
from pathlib import Path

# What opens and closes a nesting level. Angle brackets are deliberately absent:
# they are ambiguous with comparison operators, and guessing would produce more
# false alarms than real findings.
PAIRS = {")": "(", "]": "[", "}": "{"}
OPENERS = set(PAIRS.values())


def scan(text: str) -> list[str]:
    """Return one message per structural problem found."""
    problems: list[str] = []
    stack: list[tuple[str, int]] = []
    line = 1
    index = 0
    length = len(text)

    def at(newline: bool) -> None:
        nonlocal line
        if newline:
            line += 1

    while index < length:
        ch = text[index]

        if ch == "\n":
            at(True)
            index += 1
            continue

        # A line comment runs to the newline, and may contain anything.
        if text.startswith("//", index):
            end = text.find("\n", index)
            index = length if end == -1 else end
            continue

        # A block comment nests in Rust, so it needs its own depth.
        if text.startswith("/*", index):
            depth = 1
            index += 2
            while index < length and depth:
                if text.startswith("/*", index):
                    depth += 1
                    index += 2
                elif text.startswith("*/", index):
                    depth -= 1
                    index += 2
                else:
                    at(text[index] == "\n")
                    index += 1
            if depth:
                problems.append(f"line {line}: unterminated /* block comment")
            continue

        # Raw strings: r"..." or r#"..."# with any number of hashes.
        if ch == "r" and index + 1 < length and text[index + 1] in '#"':
            hashes = 0
            cursor = index + 1
            while cursor < length and text[cursor] == "#":
                hashes += 1
                cursor += 1
            if cursor < length and text[cursor] == '"':
                closer = '"' + "#" * hashes
                cursor += 1
                end = text.find(closer, cursor)
                if end == -1:
                    problems.append(f"line {line}: unterminated raw string")
                    index = length
                else:
                    at("\n" in text[cursor:end])
                    index = end + len(closer)
                continue

        # An ordinary string.
        if ch == '"':
            cursor = index + 1
            closed = False
            while cursor < length:
                if text[cursor] == "\\":
                    cursor += 2
                    continue
                if text[cursor] == '"':
                    closed = True
                    break
                at(text[cursor] == "\n")
                cursor += 1
            if not closed:
                problems.append(f"line {line}: unterminated string")
                index = length
            else:
                index = cursor + 1
            continue

        # A character literal or a lifetime. `'a'`, `'\n'`, `'\u{7f}'` are
        # literals; `'a`, `'static` are lifetimes and nest nothing.
        if ch == "'":
            cursor = index + 1
            is_literal = False
            if cursor < length and text[cursor] == "\\":
                end = text.find("'", cursor + 1)
                # A literal can hold a brace, so it has to be skipped whole.
                is_literal = end != -1 and end - cursor <= 12
                if is_literal:
                    index = end + 1
                    continue
            elif cursor + 1 < length and text[cursor + 1] == "'":
                is_literal = True
                index = cursor + 2
                continue
            elif cursor + 2 < length and text[cursor + 2] == "'":
                is_literal = True
                index = cursor + 3
                continue
            # Not a literal: a lifetime. Step past the `'` and its identifier.
            if not is_literal:
                index += 1
                continue

        if ch in OPENERS:
            stack.append((ch, line))
        elif ch in PAIRS:
            if not stack:
                problems.append(f"line {line}: `{ch}` with nothing open")
            else:
                opener, opened_at = stack.pop()
                if opener != PAIRS[ch]:
                    problems.append(
                        f"line {line}: `{ch}` closes `{opener}` opened on line {opened_at}"
                    )
        index += 1

    for opener, opened_at in stack:
        problems.append(f"line {opened_at}: `{opener}` was never closed")
    return problems


def main(argv: list[str]) -> int:
    if len(argv) < 2:
        print(__doc__.strip().splitlines()[-1].strip(), file=sys.stderr)
        return 2
    failed = False
    for name in argv[1:]:
        path = Path(name)
        if not path.is_file():
            print(f"{name}: not a file", file=sys.stderr)
            failed = True
            continue
        text = path.read_text(encoding="utf-8", errors="replace")
        problems = scan(text)
        if problems:
            failed = True
            print(f"{name}: {len(problems)} problem(s)")
            for problem in problems:
                print(f"    {problem}")
        else:
            print(f"{name}: balanced")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
