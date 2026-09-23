#!/usr/bin/env python3
"""Crates whose sources never name a dependency they declare.

Why this exists: `cargo` has no unused-dependency lint, so a manifest keeps
declaring what a refactor stopped using — a download nobody needs, a licence
obligation nobody owes, and a dependency an upstream merge will happily
re-introduce because nothing here failed when it left. The dead code in a crate
is the compiler's business (`dead_code` finds it); this is the half the compiler
cannot see.

The check is deliberately shallow: for every `[dependencies]`,
`[dev-dependencies]`, `[build-dependencies]` and `[target.*.dependencies]` entry
in `crates/*/Cargo.toml`, the package name, its hyphen-stripped form and the
same with a trailing `-rs` removed are searched for in that crate's own `src/`,
`tests/`, `benches/`, `examples/` and `build.rs`. A name that appears nowhere
there is printed.

It deliberately does not decide anything. A package's *library* name is not
always its package name (`md-5` is used as `md5::`), and a dependency may be
reachable through a re-export, a macro, a feature or a generated `include!`,
none of which this can see. So a finding is a question to answer, not a verdict:

    python tools/unused_deps.py

Exit status is 1 when there is at least one finding and 0 when there is none, so
it can be wired into a pipeline, but the answer is read by eye.
"""

from __future__ import annotations

import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

# The workspace's own crates are paths, not versions, and naming one of them is
# not a dependency declaration to check.
SKIP_PREFIXES = ("palantir-",)

DEP_SECTION = re.compile(r"^\[(?:target\.[^]]+\.)?(dependencies|dev-dependencies|build-dependencies)\]$")
DEP_LINE = re.compile(r"^([A-Za-z0-9_-]+)\s*=")
SOURCES = ("src", "tests", "benches", "examples")


def library_names(package: str) -> set[str]:
    """The identifiers a `use` of this package could be written as."""
    stripped = re.sub(r"-rs$", "", package)
    return {package.replace("-", "_"), package.replace("-", ""), stripped.replace("-", "_"), stripped}


def declared(manifest: Path) -> list[tuple[str, str]]:
    """Every dependency this manifest declares, as (package, section)."""
    found = []
    section = None
    for line in manifest.read_text(encoding="utf-8").splitlines():
        line = line.strip()
        if not line or line.startswith("#"):
            continue
        if line.startswith("["):
            section = line
            continue
        if section and DEP_SECTION.match(section):
            match = DEP_LINE.match(line)
            if match:
                found.append((match.group(1), section))
    return found


def crate_sources(crate: Path) -> str:
    """Every line of Rust this crate compiles or runs as a test."""
    paths = [crate / "build.rs"] if (crate / "build.rs").is_file() else []
    for name in SOURCES:
        directory = crate / name
        if directory.is_dir():
            paths += sorted(directory.rglob("*.rs"))
    return "\n".join(path.read_text(encoding="utf-8", errors="replace") for path in paths)


def main() -> int:
    findings = []
    for manifest in sorted(ROOT.glob("crates/*/Cargo.toml")):
        crate = manifest.parent
        text = crate_sources(crate)
        for package, section in declared(manifest):
            if package.startswith(SKIP_PREFIXES):
                continue
            if not any(re.search(r"\b" + re.escape(name) + r"\b", text) for name in library_names(package)):
                findings.append(f"{crate.relative_to(ROOT).as_posix()}: {package} ({section}) is never named")

    for finding in findings:
        print(finding)
    if not findings:
        print("no dependency is declared and never named")
        return 0
    print(f"\n{len(findings)} to read by eye: a package's lib name may differ from its package name")
    return 1


if __name__ == "__main__":
    sys.exit(main())
