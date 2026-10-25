#!/usr/bin/env python3
"""Is the rewrite lighter than what it replaces? One table, one exit code.

Why this exists: "lighter" is the condition the egui rewrite was agreed on, and
a requirement nobody measures becomes a slogan the first time a feature wants
one more texture cache. So the numbers that define it are constants below, and
every build is asked for them. The iced build is not history either -- it is
still the thing being replaced, and on the machine this was written on it is
still on disk as `dist/msvc/PalantirMC.exe`, which is exactly what the size
budget is.

What this measures itself:

* **exe size** -- one `stat` of `--exe`, against either the baseline exe named
  by `--baseline` or `MAX_EXE_BYTES`, the recorded iced size in this file.
* **forbidden dependencies** -- `cargo metadata`'s *full* graph, walked from
  the workspace's own packages over `resolve.nodes`, so `iced` cannot come back
  even as somebody else's transitive dependency. A directory with no
  `Cargo.toml`, or no `cargo` on PATH, skips this row with a note rather than
  guessing at an answer.

What it cannot measure: a size check has no desktop and no stopwatch. Idle RSS,
startup and the scrolling p95 arrive in `--measured` -- written by the
PowerShell harness in `docs/LIGHTNESS.md` -- and this tool only compares them
against the budgets. A missing measurement is SKIP, never PASS.

    python tools/lightness.py --exe dist/PalantirEgui.exe --baseline dist/PalantirMC.exe
    python tools/lightness.py --exe dist/PalantirEgui.exe --check

`--check` is what turns an exceeded budget into exit status 1. A local run
without it is a report; a run with nothing measured reports SKIPs and exits 0.
"""

from __future__ import annotations

import argparse
import json
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
DEFAULT_MEASURED = ROOT / "tools" / "lightness-measured.json"

# The iced build's release exe, as measured on 2026-10-25: `dist/msvc/
# PalantirMC.exe`, x86_64-pc-windows-msvc with the static CRT, 28,648,960
# bytes (28.6 MB). This is the budget, not a percentage of it: the first
# requirement is that the rewrite is never *bigger* than what it replaces, and
# `--baseline` re-reads the live file so a newer iced build moves the bar
# without anyone editing this number. Tightening it further is a separate
# decision, taken once the whole egui app exists.
MAX_EXE_BYTES = 28_648_960

# Idle resident memory, five minutes after the window is up and nothing is
# moving, with a panel's worth of state loaded. The iced build's own figure is
# the comparison, but 120 MB is the line the rewrite must not cross regardless.
MAX_IDLE_RSS_MB = 120

# Process start to the first window that is up, in milliseconds; the first
# painted frame is a little later, so this is the lower bound on that. A
# launcher that takes longer than this reads as broken on a warm machine.
MAX_STARTUP_MS = 400

# p95 of frame intervals while a scripted wheel scroll is running, in
# milliseconds. 16.7 ms is one frame at 60 Hz; 20 leaves a frame's worth of
# slack without accepting the iced build's 26-62 ms tail.
MAX_SCROLL_P95_MS = 20

# Packages that must not be in the egui build's graph at all -- not directly,
# not transitively. `cosmic-text` is on the list because it is the text engine
# the iced shell pulls in (vendored and patched here); egui lays its text out
# with its own shaper, so its appearance in the graph is a sign that something
# iced-shaped came along with a dependency.
FORBIDDEN_DEPS = ["iced", "iced_wgpu", "iced_winit", "cosmic-text"]


def human_bytes(count: int) -> str:
    """Bytes with separators, and the same number in decimal megabytes."""
    return f"{count:,} B ({count / 1e6:.1f} MB)"


def cargo_metadata() -> tuple[dict | None, str | None]:
    """The full metadata of the workspace in the current directory, or why not.

    `--locked` is passed whenever a `Cargo.lock` sits next to the manifest, so
    this check can never rewrite a lock file as a side effect of being run.
    A workspace without a lock (the egui slice, until its lock is committed)
    is allowed to resolve one, since that is what `cargo` does on first use.
    """
    cargo = shutil.which("cargo")
    if cargo is None:
        return None, "cargo is not on PATH; the dependency check was skipped"
    manifest = Path.cwd() / "Cargo.toml"
    if not manifest.is_file():
        return None, f"no Cargo.toml in {Path.cwd()}; the dependency check was skipped"
    command = [cargo, "metadata", "--format-version", "1"]
    if (Path.cwd() / "Cargo.lock").is_file():
        command.append("--locked")
    try:
        done = subprocess.run(
            command,
            cwd=Path.cwd(),
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="replace",
            timeout=300,
        )
    except (OSError, subprocess.TimeoutExpired) as exc:
        return None, f"cargo metadata could not run ({exc}); the dependency check was skipped"
    if done.returncode != 0:
        first = (done.stderr or "").strip().splitlines()
        reason = first[0] if first else f"exit status {done.returncode}"
        return None, f"cargo metadata failed ({reason}); the dependency check was skipped"
    try:
        return json.loads(done.stdout), None
    except json.JSONDecodeError as exc:
        return None, f"cargo metadata printed no usable JSON ({exc}); the dependency check was skipped"


def forbidden_reachable(meta: dict) -> tuple[list[str], int, str | None]:
    """Which forbidden packages this workspace can reach, and how many it walked.

    `packages` alone would be every crate in the lock file, including the ones
    for another platform or a feature nothing enables, so the walk starts at the
    workspace's own packages and follows `resolve.nodes[].deps`. Both facts
    matter: a direct `iced = ...` is the obvious failure, and `iced` arriving
    through some helper crate is the one a `--no-deps` listing would miss.
    `resolve.root` is null for a virtual workspace, which the root manifest is,
    hence `workspace_members` as the starting points.
    """
    packages = meta.get("packages") or []
    names = {package.get("id"): package.get("name") for package in packages}
    resolve = meta.get("resolve") or {}
    edges = {
        node.get("id"): [dep.get("pkg") for dep in node.get("deps") or []]
        for node in resolve.get("nodes") or []
    }
    if not edges:
        found = sorted({package.get("name") for package in packages} & set(FORBIDDEN_DEPS))
        note = "cargo metadata had no resolve graph; every listed package was checked"
        return found, len(packages), note

    starts = [member for member in meta.get("workspace_members") or [] if member in edges]
    if not starts:
        root = resolve.get("root")
        starts = [root] if root in edges else list(edges)

    seen: set[str] = set()
    found: set[str] = set()
    queue = list(starts)
    while queue:
        package = queue.pop()
        if package in seen:
            continue
        seen.add(package)
        if names.get(package) in FORBIDDEN_DEPS:
            found.add(names[package])
        queue.extend(edges.get(package, ()))
    return sorted(found), len(seen), None


def read_measured(path: Path) -> tuple[dict, list[str]]:
    """The harness's numbers, and one note for anything wrong with them.

    The file is written by a machine with a desktop, so its absence is normal on
    a CI runner: a missing file is a note, not an error. Values that are not
    numbers are treated the same way -- a hand-edited `"95"` string is the sort
    of thing that would otherwise compare as a string and pass everything.
    """
    keys = ("idle_rss_mb", "startup_ms", "scroll_p95_ms")
    if not path.is_file():
        return {}, [f"no measured file at {path}; idle RSS, startup and scroll p95 are SKIP"]
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        return {}, [f"cannot read {path} ({exc}); idle RSS, startup and scroll p95 are SKIP"]
    if not isinstance(data, dict):
        return {}, [f"{path} is not a JSON object; idle RSS, startup and scroll p95 are SKIP"]

    values: dict[str, float] = {}
    notes = []
    absent = []
    for key in keys:
        if key not in data:
            absent.append(key)
            continue
        value = data[key]
        if isinstance(value, bool) or not isinstance(value, (int, float)):
            notes.append(f"{path}: {key} is {value!r}, not a number; that budget was skipped")
            continue
        values[key] = float(value)
    if absent:
        notes.append(f"{path}: no {', '.join(absent)}; those budgets were skipped")
    return values, notes


def print_table(rows: list[tuple[str, str, str, str]]) -> None:
    """Four columns, widths taken from the rows, so nothing is ever truncated."""
    body = [("check", "measured", "budget", "status")] + rows
    widths = [max(len(row[column]) for row in body) for column in range(4)]
    for index, row in enumerate(body):
        line = "  ".join(cell.ljust(width) for cell, width in zip(row, widths))
        print(line.rstrip())
        if index == 0:
            print("  ".join("-" * width for width in widths))


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        prog="lightness.py",
        description="Check a built launcher against the lightness budgets (docs/LIGHTNESS.md).",
        epilog=(
            "Budgets: exe <= the iced baseline (or MAX_EXE_BYTES in this file); "
            f"idle RSS <= {MAX_IDLE_RSS_MB} MB; startup <= {MAX_STARTUP_MS} ms; "
            f"scroll p95 <= {MAX_SCROLL_P95_MS} ms; no {'/'.join(FORBIDDEN_DEPS)}. "
            "--check exits 1 when one is exceeded."
        ),
    )
    parser.add_argument(
        "--exe",
        type=Path,
        metavar="PATH",
        help="a built Windows exe to measure (normally the release build)",
    )
    parser.add_argument(
        "--baseline",
        type=Path,
        metavar="PATH",
        help="the current iced build's exe; its size becomes the exe budget instead of the recorded constant",
    )
    parser.add_argument(
        "--measured",
        type=Path,
        default=DEFAULT_MEASURED,
        metavar="PATH",
        help="JSON with idle_rss_mb, startup_ms and scroll_p95_ms (default: tools/lightness-measured.json)",
    )
    parser.add_argument(
        "--json-out",
        type=Path,
        metavar="PATH",
        help="also write the measurements, budgets and verdicts to this file",
    )
    parser.add_argument(
        "--check",
        action="store_true",
        help="exit 1 when a budget is exceeded (without it, an exceeded budget is reported and exit is 0)",
    )
    args = parser.parse_args(argv)

    rows: list[tuple[str, str, str, str]] = []
    notes: list[str] = []
    exceeded: list[str] = []
    measured, measured_notes = read_measured(args.measured)
    notes.extend(measured_notes)

    # The exe budget: the baseline file when one is named, the recorded iced
    # size otherwise. A path that was named and is not there is a mistake worth
    # stopping for -- silently falling back would hide a mistyped CI path.
    if args.baseline is not None:
        if not args.baseline.is_file():
            print(f"lightness: baseline is not a file: {args.baseline}", file=sys.stderr)
            return 2
        exe_budget = args.baseline.stat().st_size
        budget_source = f"baseline {args.baseline}"
    else:
        exe_budget = MAX_EXE_BYTES
        budget_source = "MAX_EXE_BYTES in tools/lightness.py"
    notes.append(f"exe budget: {human_bytes(exe_budget)} from {budget_source}")

    size = None
    if args.exe is None:
        rows.append(("exe size", "no exe given", human_bytes(exe_budget), "SKIP"))
        notes.append("no exe given: size not measured (pass --exe <built exe>)")
    elif not args.exe.is_file():
        print(f"lightness: exe is not a file: {args.exe}", file=sys.stderr)
        return 2
    else:
        size = args.exe.stat().st_size
        passed = size <= exe_budget
        rows.append(
            ("exe size", human_bytes(size), human_bytes(exe_budget), "PASS" if passed else "FAIL")
        )
        if not passed:
            exceeded.append(f"exe_bytes {size} > {exe_budget}")

    # The dependency graph, when there is one in the current directory.
    meta, reason = cargo_metadata()
    forbidden: list[str] = []
    if meta is None:
        rows.append(("dependencies", "not checked", "none", "SKIP"))
        notes.append(reason)
    else:
        forbidden, walked, walk_note = forbidden_reachable(meta)
        if walk_note:
            notes.append(walk_note)
        rows.append(
            (
                "dependencies",
                ", ".join(forbidden) if forbidden else "none",
                "none",
                "FAIL" if forbidden else "PASS",
            )
        )
        notes.append(f"dependency graph: {walked} packages reachable from {Path.cwd()}")
        if forbidden:
            exceeded.append(f"forbidden_deps {', '.join(forbidden)} > none")

    # The three numbers this tool cannot take, from the harness's JSON.
    for check, key, unit, budget, places in (
        ("idle RSS", "idle_rss_mb", "MB", MAX_IDLE_RSS_MB, 1),
        ("startup to first window", "startup_ms", "ms", MAX_STARTUP_MS, 0),
        ("scroll p95", "scroll_p95_ms", "ms", MAX_SCROLL_P95_MS, 1),
    ):
        value = measured.get(key)
        if value is None:
            rows.append((check, "not measured", f"{budget} {unit}", "SKIP"))
            continue
        passed = value <= budget
        shown = f"{value:.{places}f} {unit}"
        rows.append((check, shown, f"{budget} {unit}", "PASS" if passed else "FAIL"))
        if not passed:
            exceeded.append(f"{key} {value:.{places}f} > {budget}")

    print(f"lightness: {args.exe if args.exe else 'no exe'} against the budgets in docs/LIGHTNESS.md")
    print_table(rows)
    for note in notes:
        print(f"note: {note}")
    for line in exceeded:
        print(f"BUDGET exceeded: {line}")
    if exceeded and not args.check:
        print("run again with --check to make this a failure")

    if args.json_out is not None:
        report = {
            "exe": str(args.exe) if args.exe is not None else None,
            "exe_bytes": size,
            "baseline": str(args.baseline) if args.baseline is not None else None,
            "budgets": {
                "exe_bytes": exe_budget,
                "idle_rss_mb": MAX_IDLE_RSS_MB,
                "startup_ms": MAX_STARTUP_MS,
                "scroll_p95_ms": MAX_SCROLL_P95_MS,
                "forbidden_deps": FORBIDDEN_DEPS,
            },
            "measured": measured,
            "rows": [
                {"check": check, "measured": shown, "budget": budget, "status": status}
                for check, shown, budget, status in rows
            ],
            "forbidden_found": forbidden,
            "notes": notes,
            "exceeded": exceeded,
            "check": args.check,
            "ok": not exceeded,
        }
        try:
            args.json_out.parent.mkdir(parents=True, exist_ok=True)
            with args.json_out.open("w", encoding="utf-8", newline="\n") as handle:
                json.dump(report, handle, indent=2)
                handle.write("\n")
        except OSError as exc:
            print(f"lightness: cannot write {args.json_out} ({exc})", file=sys.stderr)
            return 2
        print(f"wrote {args.json_out}")

    return 1 if exceeded and args.check else 0


if __name__ == "__main__":
    sys.exit(main())
