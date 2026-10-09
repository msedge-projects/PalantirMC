#!/usr/bin/env python3
"""Prove this tree contains nothing we may not ship.

Six assertions, and together they are the receipt for the clean-room claim:

 1. no viral (share-alike) licence anywhere in the dependency tree;
 2. no viral-licence text in any source or text asset;
 3. every manifest row names an origin and a licence, and no origin is
    another launcher;
 4. no citation of another project's source in ours;
 5. every shipped asset and data file is listed in THIRD_PARTY_NOTICES.md;
 6. the workspace's `license` and `LICENSE` are ours to set.

Usage:
    python tools/licence_audit.py               # audit the tree holding this file
    python tools/licence_audit.py --root PATH   # audit a specific tree
    python tools/licence_audit.py --selftest    # prove the audit can fail

`--selftest` plants a copied asset, a manifest row with no licence, viral
header text, a source citation and a missing `LICENSE` in throwaway trees,
requires each to be caught, and requires a clean tree to pass. It resolves
dependencies with `cargo metadata`, so a toolchain is needed -- the same
requirement assertion 1 has.

The strings this audit hunts for are assembled from fragments below, on
purpose: this file is source, the audit scans source, and a literal here
would make the audit fail itself.
"""

from __future__ import annotations

import argparse
import fnmatch
import json
import re
import subprocess
import sys
import tempfile
from pathlib import Path

# ---------------------------------------------------------------------------
# What is forbidden. Fragments, joined at import time: the joined forms are
# what the scans look for; the fragments are all this file literally contains.
# ---------------------------------------------------------------------------

FORBIDDEN_TEXT = (
    "GP" + "L",  # the short name catches its two prefixed variants too
    "General Public " + "License",
    "copy" + "left",
)

CITATION_MARKERS = (
    "." + "vue",
    "." + "scss",
    "." + "ts:",
)

# Names of the reference application's source tree and its crates. This does
# not catch naming the service our product legitimately talks to -- its API is
# a permitted specification -- it catches pointers at someone's codebase.
REFERENCE_MARKERS = (
    "modrin" + "th-app",
    "omorp" + "hia",
    "thes" + "eus",
)

# ---------------------------------------------------------------------------
# What gets scanned
# ---------------------------------------------------------------------------

SCANNED_SUFFIXES = {
    ".rs",
    ".py",
    ".toml",
    ".lock",
    ".json",
    ".yml",
    ".yaml",
    ".html",
    ".css",
    ".js",
    ".ts",
    ".sh",
    ".svg",
    ".txt",
}
# Extensionless files are scanned too (LICENSE and friends). Markdown is
# deliberately not: the notice file has to be able to name the licences it
# excludes, and this audit documents itself in .md. What governs a document is
# assertions 3, 5 and 6.

SKIP_DIRS = {".git", "target", "dist", "__pycache__", ".scratch", ".freebuff"}

# A file is a shipped asset when it lives under one of these directories or
# carries one of these suffixes. Source and documents are ours by construction;
# everything else is somebody's file until the notice manifest says otherwise.
ASSET_DIRS = {"assets", "fonts", "fixtures", "data"}
ASSET_SUFFIXES = {
    ".png",
    ".svg",
    ".ico",
    ".icns",
    ".jpg",
    ".jpeg",
    ".gif",
    ".bmp",
    ".ttf",
    ".otf",
    ".woff",
    ".woff2",
    ".mp3",
    ".ogg",
    ".wav",
}

NOTICE_FILE = "THIRD_PARTY_NOTICES.md"
MANIFEST_HEADING = "## File manifest"

ASSERTION_LABELS = {
    1: "no viral licence in the dependency tree",
    2: "no viral-licence text in source or text assets",
    3: "manifest rows name origin and licence, no foreign launcher",
    4: "no citation of another project's source",
    5: "every shipped asset is listed in the notices",
    6: "workspace license and LICENSE are ours to set",
}


def iter_files(root: Path):
    for path in sorted(root.rglob("*")):
        if not path.is_file():
            continue
        rel = path.relative_to(root)
        if any(part in SKIP_DIRS for part in rel.parts):
            continue
        yield path, rel


def read_text(path: Path) -> str | None:
    """The file's text, or None when it is not text (binary, unreadable)."""
    try:
        return path.read_text(encoding="utf-8")
    except (UnicodeDecodeError, OSError):
        return None


def is_scanned(rel: Path) -> bool:
    return rel.suffix == "" or rel.suffix in SCANNED_SUFFIXES


def is_asset(rel: Path) -> bool:
    if any(part.lower() in ASSET_DIRS for part in rel.parts[:-1]):
        return True
    return rel.suffix.lower() in ASSET_SUFFIXES


# ---------------------------------------------------------------------------
# The six assertions
# ---------------------------------------------------------------------------


def is_share_alike(text: str) -> bool:
    """Does this licence arm carry a viral obligation?"""
    low = text.casefold()
    return any(token.casefold() in low for token in FORBIDDEN_TEXT)


def elect(expression: str):
    """SPDX-lite licence election.

    `OR` is the licensee's choice and `AND` is cumulative, so an expression
    is usable exactly when every conjunct offers at least one arm without a
    viral obligation -- and taking that arm is what this product does, with
    the election recorded in the notices. Returns the arms taken, or None
    when some conjunct offers none.
    """
    cleaned = expression.replace("(", " ").replace(")", " ")
    chosen = []
    for conjunct in re.split(r"(?i)\s+and\s+", cleaned):
        arms = [a.strip() for a in re.split(r"(?i)\s+or\s+", conjunct)]
        good = [a for a in arms if a and not is_share_alike(a)]
        if not good:
            return None
        chosen.append(good[0])
    return chosen


def audit_dependencies(root: Path, failures: dict[int, list[str]]) -> str:
    """1. Walk `cargo metadata` over licences of everything non-local."""
    try:
        proc = subprocess.run(
            ["cargo", "metadata", "--format-version", "1"],
            cwd=root,
            capture_output=True,
            text=True,
            check=False,
        )
    except FileNotFoundError:
        failures[1].append(
            "cargo is not on PATH, so the dependency tree cannot be proved"
        )
        return "cargo missing"
    if proc.returncode != 0:
        failures[1].append(f"cargo metadata failed: {proc.stderr.strip()}")
        return "cargo metadata failed"
    meta = json.loads(proc.stdout)
    members = set(meta.get("workspace_members", []))
    deps = [p for p in meta.get("packages", []) if p.get("id") not in members]
    elections = 0
    for pkg in deps:
        name = f'{pkg.get("name")} {pkg.get("version")}'
        licence = pkg.get("license")
        if not licence:
            failures[1].append(
                f"{name} names no licence; a dependency that cannot state "
                "its licence cannot be shipped"
            )
            continue
        match = elect(licence)
        if match is None:
            failures[1].append(
                f"{name} is licensed {licence}: no arm of that is free of "
                "a viral obligation, and this tree takes none"
            )
        elif " AND " in licence.upper() or " OR " in licence.upper():
            # The notices record the election; the count is the receipt.
            elections += 1
    return f"{len(deps)} dependencies, {elections} licence election(s)"


def audit_text(root: Path, failures: dict[int, list[str]]) -> str:
    """2. Scan source and text assets for viral-licence text."""
    scanned = 0
    for path, rel in iter_files(root):
        if not is_scanned(rel):
            continue
        text = read_text(path)
        if text is None:
            continue
        scanned += 1
        low = text.casefold()
        for token in FORBIDDEN_TEXT:
            if token.casefold() in low:
                failures[2].append(
                    f"{rel.as_posix()} contains forbidden text ({token})"
                )
                break
    return f"{scanned} files scanned"


def audit_citations(root: Path, failures: dict[int, list[str]]) -> str:
    """4. Scan source for pointers at another project's source."""
    checked = 0
    for path, rel in iter_files(root):
        if not is_scanned(rel):
            continue
        text = read_text(path)
        if text is None:
            continue
        checked += 1
        low = text.casefold()
        for marker in CITATION_MARKERS + REFERENCE_MARKERS:
            if marker.casefold() in low:
                failures[4].append(
                    f"{rel.as_posix()} cites another project's source "
                    f"({marker})"
                )
                break
    return f"{checked} files checked"


def parse_manifest(
    root: Path, failures: dict[int, list[str]]
) -> list[tuple[str, str, str]]:
    """Rows of the notice file's manifest table, as (path, origin, licence)."""
    notices = root / NOTICE_FILE
    if not notices.is_file():
        failures[3].append(f"{NOTICE_FILE} is missing")
        return []
    lines = (read_text(notices) or "").splitlines()
    start = None
    for i, line in enumerate(lines):
        if line.strip() == MANIFEST_HEADING:
            start = i
            break
    if start is None:
        failures[3].append(f'{NOTICE_FILE} has no "{MANIFEST_HEADING}" section')
        return []

    rows: list[tuple[str, str, str]] = []
    for line in lines[start + 1 :]:
        stripped = line.strip()
        if stripped.startswith("## "):
            break
        if not stripped.startswith("|"):
            continue
        cells = [c.strip() for c in stripped.strip("|").split("|")]
        if all(c and set(c) <= {"-", ":", " "} for c in cells):
            continue  # separator row
        if cells and cells[0].casefold() == "path":
            continue  # header row
        if len(cells) != 3:
            failures[3].append(
                f"{NOTICE_FILE} manifest row is not 'Path | Origin | Licence'"
                f": {stripped}"
            )
            continue
        rows.append((cells[0].strip("`"), cells[1], cells[2]))
    return rows


def audit_manifest(
    root: Path, failures: dict[int, list[str]]
) -> tuple[str, str]:
    """3. The manifest is complete and honest. 5. It covers every asset."""
    rows = parse_manifest(root, failures)

    # 3: every row names an origin and a licence, and no origin is a launcher.
    for where, origin, licence in rows:
        if not where or not origin or not licence:
            failures[3].append(
                f"{NOTICE_FILE} manifest row lacks a path, origin or licence:"
                f" {where or '(no path)'}"
            )
            continue
        low = origin.casefold()
        for marker in REFERENCE_MARKERS:
            if marker.casefold() in low:
                failures[3].append(
                    f"{NOTICE_FILE} claims {where} came from another launcher"
                    f" ({origin})"
                )
                break

    # 5: every shipped asset matches a row, and no row is stale.
    assets = [rel for _, rel in iter_files(root) if is_asset(rel)]
    patterns = [where for where, _, _ in rows if where]
    for rel in assets:
        name = rel.as_posix()
        if not any(fnmatch.fnmatch(name, pat) for pat in patterns):
            failures[5].append(
                f"{name} is not listed in {NOTICE_FILE}; a file the notices do"
                " not name cannot ship"
            )
    for pat in patterns:
        if not any(fnmatch.fnmatch(rel.as_posix(), pat) for rel in assets):
            failures[5].append(
                f"{NOTICE_FILE} lists {pat}, which is not in the tree"
            )
    return f"{len(assets)} assets", f"{len(patterns)} manifest rows"


def audit_our_licence(root: Path, failures: dict[int, list[str]]) -> str:
    """6. Read and print the workspace's licence declaration and LICENSE."""
    manifest = root / "Cargo.toml"
    declared = None
    if manifest.is_file():
        raw = read_text(manifest) or ""
        try:
            import tomllib

            data = tomllib.loads(raw)
            declared = data.get("workspace", {}).get("package", {}).get("license")
        except ModuleNotFoundError:  # pragma: no cover - fallback for < 3.11
            match = re.search(r'(?m)^license\s*=\s*"([^"]*)"', raw)
            declared = match.group(1) if match else None
    else:
        failures[6].append("Cargo.toml is missing")
    if not declared:
        failures[6].append("Cargo.toml declares no [workspace.package] license")

    licence_file = root / "LICENSE"
    text = read_text(licence_file) if licence_file.is_file() else None
    if text is None:
        failures[6].append("LICENSE is missing")
    elif not text.strip():
        failures[6].append("LICENSE is empty")

    present = "present" if text is not None else "MISSING"
    return f"license = {declared!r}, LICENSE {present}"


def audit(root: Path) -> tuple[dict[int, list[str]], dict[int, str]]:
    failures: dict[int, list[str]] = {n: [] for n in ASSERTION_LABELS}
    summaries = {
        1: audit_dependencies(root, failures),
        2: audit_text(root, failures),
        4: audit_citations(root, failures),
        6: audit_our_licence(root, failures),
    }
    summaries[3], summaries[5] = audit_manifest(root, failures)
    return failures, summaries


# ---------------------------------------------------------------------------
# Self-test: prove the audit can fail before trusting that it passes
# ---------------------------------------------------------------------------


def make_base_tree(root: Path) -> None:
    """A throwaway tree that must pass every assertion."""
    (root / "crates" / "probe" / "src").mkdir(parents=True)
    (root / "Cargo.toml").write_text(
        '[workspace]\nresolver = "2"\nmembers = ["crates/probe"]\n\n'
        "[workspace.package]\n"
        'license = "UNLICENSED"\n',
        encoding="utf-8",
    )
    (root / "crates" / "probe" / "Cargo.toml").write_text(
        '[package]\nname = "probe"\nversion = "0.0.0"\nedition = "2021"\n'
        "license.workspace = true\npublish = false\n",
        encoding="utf-8",
    )
    (root / "crates" / "probe" / "src" / "lib.rs").write_text(
        "// probe: a member so the throwaway workspace resolves\n",
        encoding="utf-8",
    )
    (root / "LICENSE").write_text("All rights reserved.\n", encoding="utf-8")
    (root / NOTICE_FILE).write_text(
        "# Third-party notices\n\n" + MANIFEST_HEADING + "\n\n"
        "| Path | Origin | Licence |\n| --- | --- | --- |\n",
        encoding="utf-8",
    )


def plant_copied_asset(root: Path) -> None:
    (root / "assets").mkdir()
    (root / "assets" / "copied.svg").write_text(
        '<svg xmlns="urn:probe"/>\n', encoding="utf-8"
    )


def plant_unlicensed_row(root: Path) -> None:
    (root / "assets").mkdir()
    (root / "assets" / "ours.svg").write_text(
        '<svg xmlns="urn:probe"/>\n', encoding="utf-8"
    )
    notices = root / NOTICE_FILE
    text = notices.read_text(encoding="utf-8")
    notices.write_text(
        text + "\n| `assets/ours.svg` | Us |  |\n", encoding="utf-8"
    )


def plant_viral_text(root: Path) -> None:
    (root / "crates" / "probe" / "src" / "lib.rs").write_text(
        "// " + "GP" + "L\n", encoding="utf-8"
    )


def plant_citation(root: Path) -> None:
    (root / "crates" / "probe" / "src" / "lib.rs").write_text(
        "// see ui" + "." + "v" + "ue\n", encoding="utf-8"
    )


def remove_licence_file(root: Path) -> None:
    (root / "LICENSE").unlink()


def check_expectations(
    failures: dict[int, list[str]],
    expected: list[tuple[int, str]],
    strict: bool,
) -> list[str]:
    """Every (assertion, substring) must be named by that assertion's failures.

    With `strict`, any failure beyond the expected ones is itself a problem.
    """
    problems = []
    expected_assertions = {assertion for assertion, _ in expected}
    for assertion, substr in expected:
        hits = [m for m in failures[assertion] if substr in m]
        if hits:
            continue
        problems.append(
            f"expected assertion {assertion} to fail naming {substr!r}"
        )
    if strict:
        for assertion, messages in sorted(failures.items()):
            if assertion in expected_assertions:
                continue
            for message in messages:
                problems.append(
                    f"unexpected failure from assertion {assertion}: {message}"
                )
    return problems


def selftest() -> int:
    scenarios: list[tuple[str, object, list[tuple[int, str]]]] = [
        ("a clean tree passes", None, []),
        ("one copied asset is caught", plant_copied_asset, [(5, "copied.svg")]),
        (
            "a row without a licence is caught",
            plant_unlicensed_row,
            [(3, "ours.svg")],
        ),
        ("viral text in source is caught", plant_viral_text, [(2, "lib.rs")]),
        ("a source citation is caught", plant_citation, [(4, "lib.rs")]),
        ("a missing LICENSE is caught", remove_licence_file, [(6, "LICENSE")]),
    ]
    ok = True
    with tempfile.TemporaryDirectory(prefix="licence-audit-selftest-") as tmp:
        # Licence election first: `OR` is choice, `AND` is cumulative. The
        # share-alike arm is assembled like every other hunted string.
        share_alike_arm = "L" + "GP" + "L-2.1-or-later"
        for expression, wanted in [
            ("MIT", True),
            ("MIT OR Apache-2.0", True),
            (f"MIT OR Apache-2.0 OR {share_alike_arm}", True),
            (share_alike_arm, False),
            (f"MIT AND {share_alike_arm}", False),
        ]:
            got = elect(expression) is not None
            status = "ok  " if got == wanted else "FAIL"
            print(f" {status} licence election: {expression} -> {got}")
            ok = ok and got == wanted

        for i, (name, mutate, expected) in enumerate(scenarios):
            root = Path(tmp) / f"tree-{i}"
            root.mkdir()
            make_base_tree(root)
            if mutate is not None:
                mutate(root)  # type: ignore[operator]
            failures, _ = audit(root)
            if expected:
                problems = check_expectations(failures, expected, strict=True)
            else:
                problems = [
                    f"unexpected failure from assertion {assertion}: {message}"
                    for assertion, messages in sorted(failures.items())
                    for message in messages
                ]
            status = "ok  " if not problems else "FAIL"
            print(f" {status} {name}")
            for problem in problems:
                print(f"      - {problem}")
            ok = ok and not problems
    print("self-test: " + ("all scenarios behaved" if ok else "FAILED"))
    return 0 if ok else 1


# ---------------------------------------------------------------------------
# Entry point
# ---------------------------------------------------------------------------


def report(
    root: Path, failures: dict[int, list[str]], summaries: dict[int, str]
) -> None:
    print(f"licence audit: {root}")
    for n in sorted(ASSERTION_LABELS):
        status = "ok  " if not failures[n] else "FAIL"
        print(f" {n}. {ASSERTION_LABELS[n]:<52} {status}  {summaries.get(n, '')}")
        for message in failures[n]:
            print(f"      - {message}")
    total = sum(len(messages) for messages in failures.values())
    if total:
        print(f"AUDIT FAILED: {total} problem(s)")
    else:
        print("audit passed: nothing in this tree is barred from shipping")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--root",
        type=Path,
        default=None,
        help="tree to audit (default: the tree containing this file)",
    )
    parser.add_argument(
        "--selftest",
        action="store_true",
        help="prove the audit catches planted problems in throwaway trees",
    )
    args = parser.parse_args()
    if args.selftest:
        return selftest()
    root = (args.root or Path(__file__).resolve().parent.parent).resolve()
    failures, summaries = audit(root)
    report(root, failures, summaries)
    return 1 if any(failures.values()) else 0


if __name__ == "__main__":
    sys.exit(main())
