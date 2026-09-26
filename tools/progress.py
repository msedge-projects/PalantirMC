#!/usr/bin/env python3
"""The plan's progress, counted out of the documents instead of described.

Why this exists: a percentage kept in prose drifts. The number in the last
message is right until the next slice lands, and nothing re-derives it. This
reads the two documents that do not drift for a reason -- `NEXT_STEPS.md`'s
stage table and `GATES.md`'s ledger -- and prints what they add up to, plus
how many lines the tree it is counting is made of.

What a slice is here: one gate. `GATES.md` is where a landed slice becomes
evidence (`- [x] GNN: ...`), so the ledger is the count of work that happened,
and the stage table is where the plan says which stage owns it. Every gate in
the ledger has to be attributed to exactly one stage by `GATE_OWNERS` below;
a gate that lands without a line there fails the run, and that is the drift
this tool exists to catch rather than the thing it has to be told about. The
gates written before this rewrite (G1-G57) are the shell it replaces: they are
counted, reported, and left out of the percentages.

What is an estimate: the `open` column. Each of the plan's own open items --
the bullets under "What stage N does **not** have yet" and "What stage N still
owes" -- is matched to an entry in `OPEN` here, which carries how many gates
the plan expects that item to land as. Those numbers are judgement, and they
are the only ones in this file; the tool prints them with their reasons so a
reader can disagree with one and change it. Everything else is read from the
tree.

    python tools/progress.py            # the table
    python tools/progress.py --check    # silent on success, drift on failure

On Windows, `tools/progress.cmd` is the same run for a shell where `python`
is the Microsoft Store alias or absent: it finds an interpreter itself, and it
is the entry point that keeps a double-clicked console open long enough to be
read.

Exit status is 1 when the documents disagree with each other or with this
file: a met gate no stage owns, a stage row that is `Done` with open work, an
`In progress` stage with nothing open, an open list whose items no longer match
`OPEN`, a gate `OPEN` or `GATE_OWNERS` names that the ledger does not have.
An unmet gate (`- [ ]`) is printed and counted as an open slice of the stage
that owns it; there are none at the time of writing.
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]

# What the code count reads, and what it deliberately does not. The vendored
# reference is another project's source, `dist/` is a downloaded build and
# `.scratch/` is a working directory -- counting any of them would be counting
# somebody else's lines, or the same lines twice.
CODE_DIRS = ("crates", "tools", ".github")
CODE_SKIP = {"target", "dist", ".scratch", ".git", "vendor"}
# The four files a tool writes. They are ours in the sense that this repository
# commits them, and not ours in the sense the count is for: 44,000 of the
# desktop's lines are icons and copy compiled out of the reference, and a total
# that hid that would make the hand-written tree look four times its size.
GENERATED = ("icons_gen.rs", "text_gen.rs", "theme_gen.rs", "theme_tokens.rs")

# `- [x] G83: the right panel draws ...`, and the same with an empty box.
GATE = re.compile(r"^- \[([x ])\] (G\d+[a-z]?): (.*)$")
# `| 0 | Prune what nothing references ... | **Done** |` in the stage table.
STAGE_ROW = re.compile(r"^\|\s*(\d+)\s*\|\s*(.*?)\s*\|\s*(.*?)\s*\|\s*$")
# `What stage 5 still owes, in the order it is worth doing:`
OPEN_HEADING = re.compile(r"^What stage (\d+) (?:does \*\*not\*\* have yet|still owes)(.*):\s*$")
# A plan item is bold-led: `* **Only Discover's data is real.** ...`, `1. **The
# delete.** ...`. The name between the `**` is the key `OPEN` matches on.
OPEN_ITEM = re.compile(r"^(?:\* |\d+\. )\*\*(.+?)\*\*")

# The gates each stage owns, as inclusive id ranges. The ledger is in the order
# the gates were written, which is topical rather than by stage, so the lines
# that are not a stage's plan work -- the prune, the documents, the shell this
# rewrite replaced and its design system -- are `before`, and everything a
# stage delivered is in a range below. A new gate is a new line here; the run
# fails until someone decides which stage it belongs to, which is the point.
GATE_OWNERS: dict[str, list[tuple[int, int]]] = {
    "before": [(1, 57)],  # the shell this rewrite replaces; G46b lives here too
    2: [(58, 61), (82, 82)],  # navigation, easing, copy, native; the switch
    3: [(62, 65), (76, 77), (83, 83)],  # routes, scaffold, widgets; hover; themes; the panel
    4: [(66, 75)],  # the engine, and the first page served by it
    5: [(78, 81), (84, 88)],  # create, import, the picker, the launch; the loaders; the action bar; the view-model crate; the delete
}

# The plan's open work, by the bold name its bullet carries, with the number of
# gates the plan expects the item to land as and why. The count is the tool's
# estimate and it is deliberately coarse: an item here is a family of slices
# (the loaders are the chips and the install), and it moves when the plan does.
OPEN: dict[int, list[tuple[str, int, str]]] = {
    3: [
        (
            "Only Discover's data is real.",
            5,
            "the project page, install into an instance, Skins, Servers, "
            "an instance's hosting half",
        ),
        (
            "The panel's other sections are not built.",
            3,
            "the checklist, the friends list, the news feed and its fundraiser banner",
        ),
    ],
    4: [
        (
            "The desktop's other call sites on the engine.",
            2,
            "browse.rs onto the engine, and launch.rs onto it",
        ),
    ],
    5: [
        (
            "What the old shell was the last caller of.",
            1,
            "the 199 dead items the delete exposed, and the generated table "
            "whose allowance has to come from its tool",
        ),
        (
            "The launch surface.",
            1,
            "the multi-run popover, and a download manager with more than the run's own job",
        ),
    ],
}


def parse_stages(text: str) -> list[tuple[int, str, str]]:
    """The stage table: number, what it is, and its state cell.

    Scoped to its own section rather than to every table in the document:
    the run table above it starts its rows with numbers too.
    """
    heading = "## The plan, and where each stage stands"
    body = text.split(heading, 1)
    if len(body) != 2:
        raise ValueError(f"the stage table's heading is gone: {heading!r}")
    body = body[1].split("\n## ", 1)[0]
    stages = []
    for line in body.splitlines():
        match = STAGE_ROW.match(line)
        if match is None:
            continue
        number, what, state = match.groups()
        if "**Done**" in state:
            stages.append((int(number), what, "done"))
        elif "**In progress**" in state:
            stages.append((int(number), what, "in_progress"))
        else:
            raise ValueError(f"stage {number}: state is neither Done nor In progress: {state!r}")
    return stages


def parse_ledger(text: str) -> list[tuple[str, bool, str]]:
    """The gate ledger: id, whether it is met, and its subject line."""
    gates = []
    for line in text.splitlines():
        match = GATE.match(line)
        if match is not None:
            box, gate_id, subject = match.groups()
            gates.append((gate_id, box == "x", subject))
    return gates


def parse_open_items(text: str) -> dict[int, list[str]]:
    """The plan's open items, by stage: the bold name each bullet carries."""
    open_items: dict[int, list[str]] = {}
    stage = None
    for line in text.splitlines():
        if line.startswith("#"):
            stage = None
        heading = OPEN_HEADING.match(line)
        if heading is not None:
            stage = int(heading.group(1))
            open_items.setdefault(stage, [])
            continue
        if stage is None:
            continue
        item = OPEN_ITEM.match(line)
        if item is not None:
            open_items[stage].append(item.group(1))
    return open_items


def count_lines(path: Path) -> tuple[int, int]:
    """Physical lines, and the lines that are not blank or comment-only.

    A `//`-led line is a comment and a `#`-led line is a Python one; block
    comments are not tracked, because this tree does not use them and a counter
    that guessed at `/* */` nesting would be a counter nobody could check. The
    docstrings a Python file opens with are code by this rule, which is the right
    answer for a Python file: they are the tool's own contract.
    """
    total = 0
    code = 0
    try:
        text = path.read_text(encoding="utf-8")
    except (UnicodeDecodeError, OSError):
        return (0, 0)
    for line in text.splitlines():
        total += 1
        stripped = line.strip()
        if not stripped or stripped.startswith("//") or stripped.startswith("#"):
            continue
        code += 1
    return (total, code)


def code_size(root: Path) -> tuple[list[tuple[str, str, int, int]], tuple[int, int, int, int]]:
    """What this repository owns: a row per crate, plus the tools and the total.

    Returns the rows -- name, note, physical lines, code lines -- and the four
    summary numbers, so the hand-written and generated halves can be printed as
    two totals rather than as one unrepeatable sum.
    """
    rows: list[tuple[str, str, int, int]] = []
    hand_lines = hand_code = 0
    gen_lines = gen_code = 0
    for directory in CODE_DIRS:
        base = root / directory
        if not base.is_dir():
            continue
        groups: dict[str, list[int]] = {}
        for path in sorted(base.rglob("*")):
            if not path.is_file() or path.suffix not in (".rs", ".py"):
                continue
            if CODE_SKIP & set(path.relative_to(root).parts):
                continue
            # `crates/<crate>` is one row; `tools` and `.github` are one each.
            if directory == "crates":
                relative = path.relative_to(base).parts
                name = f"crates/{relative[0]}" if relative else "crates"
            else:
                name = directory
            lines, code = count_lines(path)
            generated = path.name in GENERATED
            bucket = groups.setdefault(name, [0, 0, 0, 0])
            bucket[0] += lines
            bucket[1] += code
            bucket[2] += lines if generated else 0
            bucket[3] += code if generated else 0
            if generated:
                gen_lines += lines
                gen_code += code
            else:
                hand_lines += lines
                hand_code += code
        for name, (lines, code, generated_lines, _) in sorted(groups.items()):
            note = f"{generated_lines:,} generated" if generated_lines else ""
            rows.append((name, note, lines, code))
    return rows, (hand_lines, hand_code, gen_lines, gen_code)


def owner_of(gate_id: str) -> str | None:
    """The stage a gate belongs to, from its id."""
    number = int(re.match(r"G(\d+)", gate_id).group(1))
    for stage, ranges in GATE_OWNERS.items():
        for low, high in ranges:
            if low <= number <= high:
                return stage
    return None


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--check", action="store_true",
                        help="print nothing when the documents agree; exit 1 when they do not")
    parser.add_argument("--root", default=str(REPO),
                        help="the tree to read (default: this repository)")
    args = parser.parse_args()
    root = Path(args.root)

    next_steps = (root / "NEXT_STEPS.md").read_text(encoding="utf-8")
    gates_md = (root / "GATES.md").read_text(encoding="utf-8")

    problems: list[str] = []
    try:
        stages = parse_stages(next_steps)
        open_items = parse_open_items(next_steps)
    except ValueError as problem:
        print(f"progress: {problem}", file=sys.stderr)
        return 1
    ledger = parse_ledger(gates_md)

    stage_numbers = [number for number, _, _ in stages]
    if stage_numbers != list(range(len(stage_numbers))):
        problems.append(f"the stage table is not 0..{len(stage_numbers) - 1}: {stage_numbers}")

    # Every gate in the ledger belongs to exactly one stage, and every range
    # this file declares names a gate the ledger has.
    seen: set[str] = set()
    for stage, ranges in GATE_OWNERS.items():
        for low, high in ranges:
            for number in range(low, high + 1):
                gate_id = f"G{number}"
                if gate_id not in {gate for gate, _, _ in ledger}:
                    problems.append(f"{gate_id} is attributed to {stage} and the ledger does not have it")
    for gate_id, met, _ in ledger:
        if gate_id in seen:
            problems.append(f"{gate_id} appears in the ledger twice")
        seen.add(gate_id)
        if owner_of(gate_id) is None:
            problems.append(f"{gate_id} is in the ledger and no stage owns it -- add it to GATE_OWNERS")

    # The plan's open lists and this file's estimates are the same list, item
    # for item, or the percentage is counting something nobody asked for.
    for stage in OPEN:
        if stage not in stage_numbers:
            problems.append(f"OPEN names stage {stage}, which the stage table does not have")
    for stage, items in open_items.items():
        estimated = OPEN.get(stage)
        if estimated is None:
            problems.append(f"stage {stage} has an open list and OPEN has no estimate for it")
            continue
        names = [name for name, _, _ in estimated]
        if names != items:
            problems.append(
                f"stage {stage}: the plan's open items are {items}, OPEN estimates {names}"
            )

    met_by_stage: dict[str, int] = {}
    unmet_by_stage: dict[str, list[str]] = {}
    for gate_id, met, _ in ledger:
        stage = owner_of(gate_id)
        if stage is None:
            continue
        if met:
            met_by_stage[stage] = met_by_stage.get(stage, 0) + 1
        else:
            unmet_by_stage.setdefault(stage, []).append(gate_id)

    sized: dict[int, tuple[int, int]] = {}
    for number, _, state in stages:
        met = met_by_stage.get(number, 0)
        open_estimate = sum(gates for _, gates, _ in OPEN.get(number, ()))
        unmet = len(unmet_by_stage.get(number, ()))
        if state == "in_progress" and open_estimate + unmet == 0:
            problems.append(f"stage {number} is In progress with nothing open -- move the stage table")
        if state == "done" and open_estimate + unmet > 0:
            problems.append(f"stage {number} is Done with open work: {open_estimate + unmet} slices")
        sized[number] = (met, open_estimate + unmet)

    if problems:
        for problem in problems:
            print(f"progress: {problem}", file=sys.stderr)
        return 1

    if args.check:
        return 0

    # The table. `what` is truncated for the column; the documents hold the
    # whole sentence.
    print(f"{'stage':>5}  {'what':<52}  {'met':>4}  {'open':>4}  {'done':>5}")
    for number, what, state in stages:
        met, open_estimate = sized[number]
        if state == "done":
            percent = 100
        else:
            percent = round(100 * met / (met + open_estimate))
        column = what if len(what) <= 52 else what[:49] + "..."
        print(f"{number:>5}  {column:<52}  {met:>4}  {open_estimate:>4}  {percent:>4}%")

    # The overall is the mean of the stage numbers, each stage weighted by what
    # it holds -- its met gates plus its open items -- so a stage with more of
    # the plan in it moves the number more. A done stage with no gates in the
    # ledger still happened, so it weighs one slice rather than none.
    weights = {number: max(met + open_estimate, 1) for number, (met, open_estimate) in sized.items()}
    percents = {
        number: 100.0 if state == "done" else 100.0 * sized[number][0] / (sized[number][0] + sized[number][1])
        for number, _, state in stages
    }
    total_weight = sum(weights.values())
    overall = round(sum(percents[number] * weights[number] for number in weights) / total_weight)
    met_total = sum(met for met, _ in sized.values())
    slice_total = sum(met + open_estimate for met, open_estimate in sized.values())
    met_before = met_by_stage.get("before", 0)
    print()
    print(f"overall: {overall}%  (the stage numbers above, weighted by what each stage holds)")
    print(f"slices:  {met_total} of {slice_total} met in stages 0-{stage_numbers[-1]}; "
          f"{met_before} more gates are the shell this rewrite replaces")
    unmet = [gate_id for gate_id, met, _ in ledger if not met]
    if unmet:
        print(f"unmet:   {len(unmet)} gate(s) not met: {', '.join(unmet)}")

    print()
    print("open work, the plan's own list; the gate counts are this file's estimate:")
    for number, what, _ in stages:
        for name, gates, why in OPEN.get(number, ()):
            print(f"  stage {number}  {name:<46}  {gates:>2} gates  ({why})")

    # What the work above is made of. Physical lines first, then the lines that
    # are not blank or comment-only, because the two answer different questions:
    # the first is the file's size and the second is how much of it is a
    # statement. `vendor/` is not counted at all: it is another project's source.
    rows, (hand_lines, hand_code, gen_lines, gen_code) = code_size(root)
    print()
    print("code this repository owns, by crate (vendor/ and dist/ are not ours):")
    print(f"  {'where':<24}  {'lines':>8}  {'code':>8}  note")
    for name, note, lines, code in rows:
        print(f"  {name:<24}  {lines:>8,}  {code:>8,}  {note}")
    print(
        f"  {'hand-written total':<24}  {hand_lines:>8,}  {hand_code:>8,}  "
        "every .rs under crates/ and every .py under tools/ and .github/",
    )
    print(
        f"  {'of which generated':<24}  {gen_lines:>8,}  {gen_code:>8,}  "
        "icons_gen.rs, text_gen.rs, theme_gen.rs, theme_tokens.rs",
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
