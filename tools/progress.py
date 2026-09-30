#!/usr/bin/env python3
"""The plan's progress, counted out of the documents instead of described.

Why this exists: a percentage kept in prose drifts. The number in the last
message is right until the next slice lands, and nothing re-derives it. This
reads the two documents that do not drift for a reason -- `NEXT_STEPS.md`'s
stage table and `GATES.md`'s ledger -- and prints what they add up to -- slices,
the agent-hours those slices represent, and how many lines the tree it is
counting is made of.

What a slice is here: one gate. `GATES.md` is where a landed slice becomes
evidence (`- [x] GNN: ...`), so the ledger is the count of work that happened,
and the stage table is where the plan says which stage owns it. Every gate in
the ledger has to be attributed to exactly one stage by `GATE_OWNERS` below;
a gate that lands without a line there fails the run, and that is the drift
this tool exists to catch rather than the thing it has to be told about. The
gates written before this rewrite (G1-G57) are the shell it replaces: they are
counted, reported, and left out of the percentages.

What is an estimate: the `open` column and the two hour columns. Each of the
plan's own open items -- the bullets under "What stage N does **not** have yet"
and "What stage N still owes" -- is matched to an entry in `OPEN` here, which
carries how many gates the plan expects that item to land as and how many hours
it is expected to take. A landed slice's hours are not in the ledger either, so
the landed column prices every one at `SLICE_HOURS`, the typical slice measured
on the last three. Those numbers are judgement, and they are the only ones in
this file; the tool prints them with their reasons so a reader can disagree
with one and change it. Everything else is read from the tree.

    python tools/progress.py            # the table
    python tools/progress.py --check    # silent on success, drift on failure
    python tools/progress.py --watch    # the table again, whenever a document moves

On Windows, `tools/progress.cmd` is the same run for a shell where `python`
is the Microsoft Store alias or absent: it finds an interpreter itself, and it
is the entry point that keeps a double-clicked console open long enough to be
read.

`--watch` is for a session that is landing slices: it re-reads both documents
on an interval, redraws only when one of them has changed, and clears the
screen first, so what a reader has is the current number rather than a scroll
of every number it has been. `--check --watch` is the same pane for the rule
instead of the number -- nothing while the documents agree, and the drift
printed once, at the moment it appears.

Exit status is 1 when the documents disagree with each other or with this
file: a met gate no stage owns, a stage row that is `Done` with open work, an
`In progress` stage with nothing open, an open list whose items no longer match
`OPEN`, an estimate with no gates or no hours, a gate `OPEN` or `GATE_OWNERS`
names that the ledger does not have.
An unmet gate (`- [ ]`) is printed and counted as an open slice of the stage
that owns it; there are none at the time of writing.
"""

from __future__ import annotations

import argparse
import re
import sys
import time
from pathlib import Path

REPO = Path(__file__).resolve().parents[1]

# What the code count reads, and what it deliberately does not. The vendored
# reference is another project's source, `dist/` is a downloaded build and
# `.scratch/` is a working directory -- counting any of them would be counting
# somebody else's lines, or the same lines twice.
CODE_DIRS = ("crates", "tools", ".github")
CODE_SKIP = {"target", "dist", ".scratch", ".git", "vendor"}
# The five files a tool writes. They are ours in the sense that this repository
# commits them, and not ours in the sense the count is for: most of the desktop
# crate's lines are icons or copy compiled out of the reference, and a total that
# hid that would make the hand-written tree look several times its size. The
# newest of them is `locale_gen.rs` (G120/G121): one sparse table per locale,
# 89,459 lines, which this tuple did not know about until the locales' ledger and
# so counted as hand-written in the two commits before it.
GENERATED = (
    "icons_gen.rs",
    "locale_gen.rs",
    "text_gen.rs",
    "theme_gen.rs",
    "theme_tokens.rs",
)

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
    "before": [(1, 57), (92, 92)],  # the shell this rewrite replaces, and the dashboard; G46b is here too
    2: [(58, 61), (82, 82)],  # navigation, easing, copy, native; the switch
    3: [(62, 65), (76, 77), (83, 83), (96, 98), (101, 111), (115, 118), (120, 124), (125, 125), (128, 128), (129, 129)],  # routes, scaffold, widgets; hover; themes; the panel; the project page's own documents and its installs; the panel's news feed and its checklist with the friends sentence beside it, Home's welcome screen as the page, the Skins page's read of the account's own skins and its write half, the measurement of what the four surfaces left need, the profile page read off Modrinth's published API, the measurement of what a Modrinth session is and would reach, the check of that page's two documents against the live service, and the reading of the api-client that writes the Servers API down; the reference's own locales -- their compiled tables, the setting that reads one, and the ledger of what the other 32 carry against English (G120-G122); the Skins page's upload half -- the picker, the padding to 64x64, the arm style read from the texture's own pixels, the multipart body and the flow that joins them (G123), and the store the reader's own additions are kept in with the editor that changes one -- arm style, cape, the Ears notice and the way to take a skin off (G124); the instance settings modal's own settings half -- the heap, the Java path and the JVM arguments an instance *file* already holds, read back into controls behind the reference's own override switches and written on save, which is the write side `model.rs` inherited without a control (G125); the instance settings modal's installation tab -- the platform, the game version and the loader build the instance's own `mmc-pack.json` holds, read through `crate::pack`, written back to the same profile, with every other loader taken out of it and a loader this launcher does not model refused rather than deleted (G128), and the pack such an instance came from -- the link a pack install leaves beside the profile, the card named from the project document and the version list rather than from the file, and *Unlink modpack*, the one action of the reference's four that takes the link and leaves everything else (G129); and what the interface costs at size, which is the pages' own per-frame cost and is why it is stage 3's rather than the shell's -- its worst number was the instance page's Files tab, the one read that turned out to be worse than linear fixed (G115) the page's whole listing taken out of the draw path afterwards (G116) and the tab body windowed so that a frame draws the rows on screen rather than the folder (G117); and the decision that stops the four Modrinth *account* surfaces from being owed at all -- Hosting and its billing, the Share tab, the skin store and the signed-in friends half are dropped rather than deferred, with the sentence on each of them saying which service it is (G118) (G99-G100, the Forge installers and their processors, are stage 4's)
    4: [(66, 75), (91, 91), (93, 95), (99, 100), (107, 107), (112, 114)],  # the engine; the first page on it; the launch's and a pack's transfers; the loaders' own profiles and Minecraft's own file; the Forge-shaped installers' own metadata and their processors; the measurement of what the mirror serves for those two uids instead of them; and the engine's own error path keeping the service's sentence beside the status (G92 is the dashboard, owned by `before`)
    5: [(78, 81), (84, 90), (126, 127)],  # create, import, the picker, the launch; the loaders; the action bar; the view-model crate; the delete; the prune; the several runs; the loader's own install reached from a launch -- the Forge-shaped loaders' processors run before the resolve, which G100 left unreached from the interface and G107 named the order for (G126); and the flip that follows it in that order, sending the two Forge-shaped uids to the installer's own translated profile rather than to Prism's ForgeWrapper rewrite (G127; G125 went to the instance-settings modal first)
}

# What a landed slice is priced at, because the ledger records what was built
# and not how long it took. Measured on the last three landed (G115-G117): each
# took about 2-3 h of agent wall-clock, roughly 25 min of that spent waiting on
# local compiles. A range rather than a number because a slice is not a fixed
# amount of work, and one value for every stage because nothing in the documents
# says one stage's slices are cheaper than another's.
SLICE_HOURS = (2.0, 3.0)

# The plan's open work, by the bold name its bullet carries: how many gates the
# plan expects the item to land as, the hours it is expected to take, and why.
# Both numbers are the tool's estimate and they are deliberately coarse: an
# item here is a family of slices (the loaders are the chips and the install),
# and they move when the plan does.
OPEN: dict[int, list[tuple[str, int, tuple[float, float], str]]] = {
    3: [
        (
            "The installation tab's pack actions are not built.",
            1,
            (2.0, 4.0),
            "the three actions the reference's linked-modpack panel carries that "
            "re-run an install: *Repair instance*, *Re-install modpack* and "
            "*Change version* (its *Swap*, a pack version installed over the "
            "instance's own). G129 landed the bookkeeping they need -- the link a "
            "pack install leaves and the card named from it -- and *Unlink "
            "modpack*, which is the one of the four that is a file write rather "
            "than an install. What is left is `crate::install` reached from the "
            "modal, needing no third party, with the sync-override and sharing "
            "tabs beside it the account services G118 dropped",
        ),
        (
            "The Skins page's edit half is not built.",
            1,
            (2.0, 3.0),
            "the modal beside the account's own rows: reorder the skins it owns, "
            "take one off (`unequip_skin`, which names the modal as its only "
            "control) and keep the texture a reader uploaded. G123 landed the "
            "picker, the pad to 64x64, the arm style read from the pixels and the "
            "multipart body; the *store* half that used to be part of this item is "
            "Modrinth's and went with G118",
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


def plan_totals(
    stages: list[tuple[int, str, str]],
    sized: dict[int, tuple[int, int]],
    met_by_stage: dict[str, int],
) -> dict[str, int | dict[int, float]]:
    """The two headline numbers, and the per-stage percentages behind them.
    
    A function rather than a block inside `report`, because the dashboard asks
    the same question and a page that computed its own answer could disagree
    with the pane that was left open beside it.

    The overall is the mean of the stage numbers, each stage weighted by what it
    holds -- its met gates plus its open items -- so a stage with more of the
    plan in it moves the number more. A done stage with no gates in the ledger
    still happened, so it weighs one slice rather than none.
    """
    weights = {number: max(met + open, 1) for number, (met, open) in sized.items()}
    percents = {
        number: 100.0 if state == "done"
        else 100.0 * sized[number][0] / (sized[number][0] + sized[number][1])
        for number, _, state in stages
    }
    total_weight = sum(weights.values())
    return {
        "overall": round(sum(percents[number] * weights[number] for number in weights) / total_weight),
        "percents": percents,
        "met_total": sum(met for met, _ in sized.values()),
        "slice_total": sum(met + open for met, open in sized.values()),
        "met_before": met_by_stage.get("before", 0),
    }


def hours_form(low: float, high: float) -> str:
    """`3-5 h` for a range, `2 h` for a point, `-` when there is nothing.

    Rounded to a tenth because the inputs are estimates: a division of `5-8 h`
    over three agents is `1.7-2.7 h` and not `1.6666666666666667-2.6666666...`,
    which is what a float prints and what makes an estimate look like a
    measurement.
    """
    if high <= 0:
        return "-"
    low, high = round(low, 1), round(high, 1)
    if low == high:
        return f"{low:g} h"
    return f"{low:g}-{high:g} h"


def effort(
    stages: list[tuple[int, str, str]],
    sized: dict[int, tuple[int, int]],
    unmet_by_stage: dict[int, list[str]],
) -> dict[int, tuple[tuple[float, float], tuple[float, float]]]:
    """Hours per stage: `(landed low, landed high), (left low, left high)`.

    Two different estimates on purpose. A *landed* stage is priced per slice at
    `SLICE_HOURS`, because the ledger records what was built and not how long it
    took, and that rate is the only measurement this file has. An *open* stage
    is priced at what its own `OPEN` entries say, because that is the plan's
    answer to "how much is this" rather than an average of other work. A stage
    with an unmet gate -- none at the time of writing -- is priced the first way
    for it too, because a gate is a slice whether or not it has landed.

    A function rather than a block in `report`, for the same reason as
    `plan_totals`: the dashboard shows these numbers too, and a second
    computation is a second answer.
    """
    hours: dict[int, tuple[tuple[float, float], tuple[float, float]]] = {}
    for number, _, _ in stages:
        met = sized[number][0]
        landed = (met * SLICE_HOURS[0], met * SLICE_HOURS[1])
        unmet = len(unmet_by_stage.get(number, ()))
        low = sum(entry[2][0] for entry in OPEN.get(number, ())) + unmet * SLICE_HOURS[0]
        high = sum(entry[2][1] for entry in OPEN.get(number, ())) + unmet * SLICE_HOURS[1]
        hours[number] = (landed, (low, high))
    return hours


def owner_of(gate_id: str) -> str | None:
    """The stage a gate belongs to, from its id."""
    number = int(re.match(r"G(\d+)", gate_id).group(1))
    for stage, ranges in GATE_OWNERS.items():
        for low, high in ranges:
            if low <= number <= high:
                return stage
    return None


def report(root: Path, *, agents: int = 3) -> tuple[str, list[str]]:
    """The table as text, and the disagreements that stop it being printed.

    `agents` is only the divisor the effort line uses: how many agents are
    working the plan is a fact about the session, not about the documents, so it
    is a flag with a default rather than something the documents could be read
    for.

    A function rather than the body of `main`, because `--watch` asks for a
    fresh report on every change: the loop has to be able to render again
    without inheriting half of the last one.
    """
    next_steps = (root / "NEXT_STEPS.md").read_text(encoding="utf-8")
    gates_md = (root / "GATES.md").read_text(encoding="utf-8")

    problems: list[str] = []
    stages = parse_stages(next_steps)
    open_items = parse_open_items(next_steps)
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
        for name, gates, span, _ in OPEN[stage]:
            if gates < 1 or span[1] <= 0 or span[1] < span[0]:
                problems.append(
                    f"stage {stage}: {name!r} has an impossible estimate: {gates} gate(s), {span}"
                )
    for stage, items in open_items.items():
        estimated = OPEN.get(stage)
        if estimated is None:
            problems.append(f"stage {stage} has an open list and OPEN has no estimate for it")
            continue
        names = [name for name, _, _, _ in estimated]
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
        open_estimate = sum(gates for _, gates, _, _ in OPEN.get(number, ()))
        unmet = len(unmet_by_stage.get(number, ()))
        if state == "in_progress" and open_estimate + unmet == 0:
            problems.append(f"stage {number} is In progress with nothing open -- move the stage table")
        if state == "done" and open_estimate + unmet > 0:
            problems.append(f"stage {number} is Done with open work: {open_estimate + unmet} slices")
        sized[number] = (met, open_estimate + unmet)

    if problems:
        return "", problems

    hours = effort(stages, sized, unmet_by_stage)
    landed = (
        sum(pair[0][0] for pair in hours.values()),
        sum(pair[0][1] for pair in hours.values()),
    )
    left = (
        sum(pair[1][0] for pair in hours.values()),
        sum(pair[1][1] for pair in hours.values()),
    )

    out: list[str] = []
    # The table. `what` is truncated for the column; the documents hold the
    # whole sentence. The landed hours are a rate rather than a recording --
    # see `SLICE_HOURS` -- and the left ones are the plan's own estimates.
    out.append(
        f"{'stage':>5}  {'what':<52}  {'met':>4}  {'open':>4}  {'done':>5}  "
        f"{'landed':>9}  {'left':>8}"
    )
    for number, what, state in stages:
        met, open_estimate = sized[number]
        if state == "done":
            percent = 100
        else:
            percent = round(100 * met / (met + open_estimate))
        column = what if len(what) <= 52 else what[:49] + "..."
        stage_landed, stage_left = hours[number]
        out.append(
            f"{number:>5}  {column:<52}  {met:>4}  {open_estimate:>4}  {percent:>4}%  "
            f"{hours_form(*stage_landed):>9}  {hours_form(*stage_left):>8}"
        )

    totals = plan_totals(stages, sized, met_by_stage)
    overall = totals["overall"]
    met_total = totals["met_total"]
    slice_total = totals["slice_total"]
    met_before = met_by_stage.get("before", 0)
    out.append("")
    out.append(f"overall: {overall}%  (the stage numbers above, weighted by what each stage holds)")
    out.append(f"slices:  {met_total} of {slice_total} met in stages 0-{stage_numbers[-1]}; "
               f"{met_before} more gates are the shell this rewrite replaces")
    unmet = [gate_id for gate_id, met, _ in ledger if not met]
    if unmet:
        out.append(f"unmet:   {len(unmet)} gate(s) not met: {', '.join(unmet)}")
    out.append(
        f"effort:  {met_total} slices landed = {hours_form(*landed)} of agent time, "
        f"{hours_form(landed[0] / agents, landed[1] / agents)} per agent at the typical "
        f"{hours_form(*SLICE_HOURS)} a slice (measured on the last three, G115-G117)"
    )
    if slice_total > met_total:
        out.append(
            f"         {slice_total - met_total} open slices left = {hours_form(*left)}; "
            f"over {agents} agents = {hours_form(left[0] / agents, left[1] / agents)} each, "
            "unevenly, because the documents and the one CI ref are shared"
        )
    else:
        out.append("         nothing open, so no hours remain")

    out.append("")
    out.append("open work, the plan's own list; the gate counts and hours are this file's estimate:")
    for number, what, _ in stages:
        for name, gates, span, why in OPEN.get(number, ()):
            plural = "s" if gates != 1 else " "
            out.append(
                f"  stage {number}  {name:<46}  {gates:>2} gate{plural}  "
                f"{hours_form(*span):>8}  ({why})"
            )

    # What the work above is made of. Physical lines first, then the lines that
    # are not blank or comment-only, because the two answer different questions:
    # the first is the file's size and the second is how much of it is a
    # statement. `vendor/` is not counted at all: it is another project's source.
    rows, (hand_lines, hand_code, gen_lines, gen_code) = code_size(root)
    out.append("")
    out.append("code this repository owns, by crate (vendor/ and dist/ are not ours):")
    out.append(f"  {'where':<24}  {'lines':>8}  {'code':>8}  note")
    for name, note, lines, code in rows:
        out.append(f"  {name:<24}  {lines:>8,}  {code:>8,}  {note}")
    out.append(
        f"  {'hand-written total':<24}  {hand_lines:>8,}  {hand_code:>8,}  "
        "every .rs under crates/ and every .py under tools/ and .github/",
    )
    out.append(
        f"  {'of which generated':<24}  {gen_lines:>8,}  {gen_code:>8,}  "
        "icons_gen.rs, locale_gen.rs, text_gen.rs, theme_gen.rs, theme_tokens.rs",
    )
    return "\n".join(out) + "\n", []


def document_signature(root: Path) -> tuple[tuple[int, int], ...]:
    """`(mtime, size)` per document, so that a save is seen as a change.

    mtime alone is not enough: two saves inside one filesystem timestamp tick
    are a thing editors do, and the second would be missed. A document caught
    mid-save -- missing, or truncated by the editor that is writing it -- is a
    state like any other, and the redraws on the way in and the way out are how
    the pane recovers.
    """
    signature = []
    for name in ("NEXT_STEPS.md", "GATES.md"):
        try:
            info = (root / name).stat()
        except OSError:
            signature.append((-1, -1))
        else:
            signature.append((info.st_mtime_ns, info.st_size))
    return tuple(signature)


def watch(root: Path, *, check: bool, interval: float, agents: int = 3) -> int:
    """`report` on every change to the documents, until interrupted.

    A pane rather than a print: the table is cleared and redrawn, so a reader
    watching a slice land has the current number instead of a scroll of every
    number it has been. When stdout is not a terminal -- a log, a pipe -- the
    clear is left out and each redraw is announced with the timestamped header
    alone, because a file full of escape codes is not a readable log.

    `--check` keeps its meaning here: nothing at all while the documents agree,
    and the disagreement printed the moment it appears. It goes on watching
    afterwards, so that fixing the plan clears the pane the same way any other
    change does.
    """
    watched = ("NEXT_STEPS.md", "GATES.md")
    tty = sys.stdout.isatty()
    stamp: tuple[tuple[int, int], ...] | None = None
    verdict: bool | None = None
    try:
        while True:
            state = document_signature(root)
            if state == stamp:
                time.sleep(interval)
                continue
            stamp = state
            now = time.strftime("%H:%M:%S")
            try:
                text, problems = report(root, agents=agents)
            except (ValueError, OSError) as problem:
                text, problems = "", [str(problem)]
            if check:
                if bool(problems) != verdict:
                    verdict = bool(problems)
                    print(f"progress --check  {now}  {'drift' if verdict else 'ok'}")
                    for problem in problems:
                        print(f"progress: {problem}", file=sys.stderr)
                    # A verdict with no flush is a verdict nobody sees: this
                    # branch prints once and then goes quiet, and a pane that
                    # has stopped writing is a pane with its output still on a
                    # buffer.
                    sys.stdout.flush()
            else:
                if tty:
                    sys.stdout.write("\x1b[2J\x1b[H")
                print(f"progress --watch  {now}  (every {interval:g}s; Ctrl-C to stop)")
                stamps = ", ".join(
                    f"{name} {time.strftime('%H:%M:%S', time.localtime(mtime / 1e9))}"
                    if mtime > 0 else f"{name} (missing)"
                    for name, (mtime, _) in zip(watched, state)
                )
                print(f"watching {stamps}")
                if problems:
                    print("the documents do not agree:")
                    for problem in problems:
                        print(f"  {problem}")
                else:
                    sys.stdout.write(text)
                sys.stdout.flush()
            time.sleep(interval)
    except KeyboardInterrupt:
        # Ctrl-C ends the pane, and the prompt wants the newline a cleared
        # screen has been holding back.
        sys.stdout.write("\n")
        return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--check", action="store_true",
                        help="print nothing when the documents agree; exit 1 when they do not")
    parser.add_argument("--watch", action="store_true",
                        help="redraw whenever NEXT_STEPS.md or GATES.md changes")
    parser.add_argument("--interval", type=float, default=1.0, metavar="SECONDS",
                        help="how often --watch looks for a change (default: 1)")
    parser.add_argument("--agents", type=int, default=3, metavar="N",
                        help="how many agents the remaining hours are divided by "
                             "(default: 3)")
    parser.add_argument("--dashboard", nargs="?", const="", metavar="PATH",
                        help="write the HTML dashboard instead of the table "
                             "(default: .scratch/progress.html)")
    parser.add_argument("--root", default=str(REPO),
                        help="the tree to read (default: this repository)")
    args = parser.parse_args()
    if args.agents < 1:
        # Only the effort line divides by this, and a division by zero in the
        # middle of the table is a bad way to find out the flag was mistyped.
        parser.error("--agents takes at least one agent")
    root = Path(args.root)

    if args.dashboard is not None:
        # Imported here rather than at the top: the terminal report is the tool
        # and this is the other face of it, and a page nobody asked for is not a
        # cost the common case should pay.
        import dashboard

        out = Path(args.dashboard) if args.dashboard else root / ".scratch" / "progress.html"
        page, problems = dashboard.render(root)
        if problems:
            for problem in problems:
                print(f"progress: {problem}", file=sys.stderr)
            return 1
        out.parent.mkdir(parents=True, exist_ok=True)
        out.write_text(page, encoding="utf-8")
        # The path rather than only a byte count: the whole point of writing a
        # file is that the caller can open it, and the shell that opened it is
        # not always the shell that asked for it.
        print(f"wrote {out}  ({len(page):,} bytes)")
        return 0

    if args.watch:
        # A floor rather than a reject: a poll this tool owes nothing to being
        # fast at, and `--interval 0` from a script should not spin a core.
        return watch(root, check=args.check, interval=max(args.interval, 0.1), agents=args.agents)

    try:
        text, problems = report(root, agents=args.agents)
    except (ValueError, OSError) as problem:
        print(f"progress: {problem}", file=sys.stderr)
        return 1
    if problems:
        for problem in problems:
            print(f"progress: {problem}", file=sys.stderr)
        return 1
    if args.check:
        return 0
    print(text, end="")
    return 0


if __name__ == "__main__":
    sys.exit(main())
