#!/usr/bin/env python3
"""Which Lucide release each icon in the reference's set came from, by geometry.

The reference's icons are not the reference's drawings. Every SVG under
`vendor/modrinth-app/assets/icons` carries Lucide's own header comment when its
build kept one -- `<!-- @license lucide-static v0.562.0 - ISC -->` -- and the
number in it is the release that file was taken from. But only 74 of the 313
carry the comment at all: the set accumulated from many releases and their build
re-serialises what it takes, so the comment is neither present everywhere nor
evidence where it is.

String comparison cannot tell those two facts apart. `check.svg` is the case
that proves it:

    theirs:          <path d="M20 6L9 17l-5-5" />
    Lucide 0.562.0:  <path d="M20 6 9 17l-5-5" />

An implicit `lineto` written out in full against one left implicit, and the same
drawing either way. So this tool does not compare text. It reads both files
through **`tools/gen_icons.py`'s own reader** -- the same parser, the same
arc-to-cubic conversion, the same inheritance -- and compares the geometry that
comes out. A match then means what the licence question needs it to mean: the
icon in the vendored tree is that Lucide release's drawing, and the copy came
through the reference rather than from Lucide.

That distinction is the whole point. The icons are ISC-licensed and this
repository is GPL-3.0-only, so re-sourcing them from Lucide changes no right
anyone has -- what it changes is *whose copy* is in the tree, which is the
question this tree's own provenance documents exist to answer.

## What the comparison finds, and what it cannot

A match is proved; a non-match is not. Three things can make an icon that is
Lucide's own drawing fail to match:

* a release the ladder did not sample. The ladder is a sample -- see
  `LADDER_STEP` -- and the release this tool records is the earliest *sampled*
  release holding that geometry, not the release it was taken from. A tighter
  ladder (`--step 1`) is slower and answers more precisely.
* a file their build reshaped rather than re-serialised: a `viewBox` changed, a
  stroke width edited, a path simplified.
* a drawing that is theirs. Some of the set is: the loader marks under
  `tags/loaders` are other projects' logos rather than Lucide's at all, and a
  band of the top-level names (`affiliate`, `client`, `dropdown`, `gap`,
  `omorphia`, `page-round`, `spinner`, `unknown`, `updated`, ...) do not exist in
  Lucide under any release.

So the output is a two-column answer, and the second column is a work list
rather than a verdict.

## Running it

    python tools/lucide_source.py --prove            # fetch, compare, print the split
    python tools/lucide_source.py --prove --json .scratch/lucide.json
    python tools/lucide_source.py --icon check       # one icon, every sampled release
    python tools/lucide_source.py --vendor           # write vendor/lucide-icons/ + UPSTREAM.md
    python tools/lucide_source.py --check            # verify a vendored tree against its own record

`--prove` needs the network the first time and a cached release after that; the
cache lives in `.scratch/lucide/`, which is git-ignored, and `--check` and
`--vendor` read only what is there. Nothing here runs in the build: `gen_icons.py`
reads the vendored tree, and this tool is what puts the material in it.
"""

from __future__ import annotations

import argparse
import hashlib
import io
import json
import re
import sys
import tarfile
import urllib.request
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import gen_icons  # noqa: E402  -- the reader, re-used rather than re-written

ROOT = Path(__file__).resolve().parent.parent
REFERENCE = ROOT / "vendor" / "modrinth-app" / "assets" / "icons"
CACHE = ROOT / ".scratch" / "lucide"
OUT = ROOT / "vendor" / "lucide-icons"
TARBALL = "https://registry.npmjs.org/lucide-static/-/lucide-static-{release}.tgz"
VERSIONS = "https://registry.npmjs.org/lucide-static"

# The three directories the reference keeps under `assets/icons`, as
# `gen_icons.py` walks them, and the same three this tool walks in the vendored
# tree it writes. Mirrored rather than re-derived so that a repointed reader
# needs no mapping table: `tags/categories/badge-check.svg` has one place it can
# be, in both trees.
SETS = ("", "tags/categories", "tags/loaders")

# How the ladder is sampled out of the registry's version list. Lucide's icons
# change on the release that redraws them and not again for a long time -- the
# sample this tool shipped with found the same geometry in eleven consecutive
# releases for `chevron-up` -- so every eighth minor is enough to place an icon
# to within a handful of releases, and `--step 1` is there for the icons it
# cannot place at all.
LADDER_STEP = 8
LADDER_FROM = 100
# Their own manifest pins this one (`assets/package.json`), so it is in the
# ladder whatever the step is: the release the reference says it takes icons from
# is the one a re-source should be able to name.
LADDER_ALWAYS = ("0.562.0",)


def registry() -> list[str]:
    """Every published `lucide-static` release, oldest first."""
    cached = CACHE / "versions.json"
    if not cached.is_file():
        CACHE.mkdir(parents=True, exist_ok=True)
        with urllib.request.urlopen(VERSIONS, timeout=60) as response:
            cached.write_bytes(response.read())
    document = json.loads(cached.read_text(encoding="utf-8"))
    releases = [v for v in document["versions"] if re.fullmatch(r"0\.\d+\.\d+", v)]
    return sorted(releases, key=key)


def key(release: str) -> tuple[int, ...]:
    return tuple(int(part) for part in release.split("."))


def ladder(step: int) -> list[str]:
    """The releases to sample, as every `step`-th minor's highest patch."""
    minors: dict[int, list[str]] = {}
    for release in registry():
        minors.setdefault(key(release)[1], []).append(release)
    picked = [max(minors[minor], key=key) for minor in sorted(minors) if minor >= LADDER_FROM]
    picked = picked[::step]
    for always in LADDER_ALWAYS:
        if always not in picked and any(key(r)[1] == key(always)[1] for r in registry()):
            picked.append(always)
    return sorted(set(picked), key=key)


def fetch(release: str) -> Path:
    """One release's icons, extracted into the cache. Never re-downloads."""
    directory = CACHE / release / "icons"
    if directory.is_dir() and any(directory.iterdir()):
        return directory
    directory.mkdir(parents=True, exist_ok=True)
    with urllib.request.urlopen(TARBALL.format(release=release), timeout=180) as response:
        payload = response.read()
    with tarfile.open(fileobj=io.BytesIO(payload), mode="r:gz") as archive:
        members = [m for m in archive.getmembers()
                   if m.name.startswith("package/icons/") and m.name.endswith(".svg")]
        if not members:
            raise SystemExit(f"{release}: no icons in the package")
        for member in members:
            # Flattened into the cache rather than kept as a package tree: the
            # release is already the directory, and a `package/` level inside it
            # is a fact about npm's tarballs rather than about the icons.
            (directory / Path(member.name).name).write_bytes(archive.extractfile(member).read())
        # The ISC text travels with the icons it governs, so it is taken from the
        # same tarball rather than believed: Lucide's `package/LICENSE` is the
        # grant, and the copy that ships in `vendor/lucide-icons/` is this one.
        licence = archive.getmember("package/LICENSE")
        (CACHE / release / "LICENSE").write_bytes(archive.extractfile(licence).read())
    return directory


def signature(path: Path) -> tuple | None:
    """The geometry of one SVG, in the shape `gen_icons.py` would emit it.

    `None` for a file the reader refuses, which is not a mismatch: four tag
    icons are refused by name in `gen_icons.REFUSED_TAGS` and are absent from
    the generated file as well, so they need no source of their own.
    """
    discard = ([], [], [], [])
    try:
        box, elements = gen_icons.read_icon(path, str(path), *discard)
    except gen_icons.Unsupported:
        return None
    except (ValueError, IndexError):
        return None
    return (
        box,
        tuple(
            (
                round(element.stroke_width, 4),
                element.filled,
                element.ink,
                element.opacity,
                element.even_odd,
                tuple(
                    command if command[0] == "Close"
                    else (command[0],) + tuple(
                        round(value, 3) for value in command[1:]
                    )
                    for command in element.commands
                ),
            )
            for element in elements
        ),
    )


def reference_icons() -> list[tuple[str, Path]]:
    """Every icon in the vendored reference tree, as (name below the root, path)."""
    found: list[tuple[str, Path]] = []
    for directory in SETS:
        where = REFERENCE / directory if directory else REFERENCE
        for path in sorted(where.glob("*.svg")):
            name = f"{directory}/{path.name}" if directory else path.name
            found.append((name, path))
    return found


def index(release: str) -> dict[tuple, str]:
    """A release's icons, keyed by geometry, so a match is a lookup."""
    directory = fetch(release)
    by_geometry: dict[tuple, str] = {}
    for path in sorted(directory.glob("*.svg")):
        geometry = signature(path)
        if geometry is not None and geometry not in by_geometry:
            by_geometry[geometry] = path.stem
    return by_geometry


def prove(releases: list[str], only: str | None) -> dict[str, dict]:
    """Where every icon in the reference's set comes from, by geometry."""
    icons = reference_icons()
    if only:
        icons = [(n, p) for n, p in icons if Path(n).stem == only]
        if not icons:
            raise SystemExit(f"no icon named {only} in {REFERENCE}")
    print(f"reference tree: {len(icons)} icons", file=sys.stderr)
    tables: dict[str, dict[tuple, str]] = {}
    for position, release in enumerate(releases, start=1):
        tables[release] = index(release)
        print(f"  [{position}/{len(releases)}] {release}: {len(tables[release])} icons",
              file=sys.stderr)

    found: dict[str, dict] = {}
    for name, path in icons:
        geometry = signature(path)
        if geometry is None:
            found[name] = {"release": None, "lucide": None, "reason": "refused by the reader"}
            continue
        hits = [release for release in releases if geometry in tables[release]]
        if hits:
            release = hits[0]
            found[name] = {
                "release": release,
                "lucide": tables[release][geometry],
                "reason": None,
            }
        else:
            found[name] = {"release": None, "lucide": None, "reason": "no sampled release"}
    return found


def vendor(found: dict[str, dict], releases: list[str]) -> None:
    """Write the proved icons into `vendor/lucide-icons/`, and say what it is."""
    written = 0
    used = sorted({row["release"] for row in found.values() if row["release"]}, key=key)
    for name, row in sorted(found.items()):
        if not row["release"]:
            continue
        source = CACHE / row["release"] / "icons" / f"{row['lucide']}.svg"
        target = OUT / name
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(source.read_bytes())
        written += 1
    print(f"vendored {written} of {len(found)} icons into {OUT.relative_to(ROOT)}")
    licence = CACHE / used[-1] / "LICENSE"
    if licence.is_file():
        (OUT / "LICENSE").write_bytes(licence.read_bytes())
        print(f"licence: {licence.relative_to(ROOT)} -> {(OUT / 'LICENSE').relative_to(ROOT)}")
    missing = sorted(name for name, row in found.items() if not row["release"])
    print(f"left in the reference tree: {len(missing)}")
    for name in missing:
        print(f"  {name}\t{found[name]['reason']}")


def check() -> int:
    """Every vendored file still being the bytes its record names."""
    record = OUT / "UPSTREAM.md"
    if not record.is_file():
        print(f"no {record.relative_to(ROOT)}: run --vendor first", file=sys.stderr)
        return 2
    digest = re.compile(r"^\| `([^`]+)` \| `([^`]+)` \| `([0-9a-f]{64})` \|$", re.M)
    rows = digest.findall(record.read_text(encoding="utf-8"))
    if not rows:
        print(f"{record.relative_to(ROOT)}: no rows read", file=sys.stderr)
        return 2
    wrong = 0
    for name, release, expected in rows:
        path = OUT / name
        if not path.is_file():
            print(f"missing: {name}")
            wrong += 1
            continue
        actual = hashlib.sha256(path.read_bytes()).hexdigest()
        if actual != expected:
            print(f"changed: {name}\n  record {expected}\n  file   {actual}")
            wrong += 1
    print(f"{len(rows)} rows, {wrong} wrong")
    return 1 if wrong else 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--prove", action="store_true",
                        help="fetch the ladder, compare geometry, print the split")
    parser.add_argument("--json", type=Path, help="write the table as JSON")
    parser.add_argument("--icon", help="one icon, against every sampled release")
    parser.add_argument("--step", type=int, default=LADDER_STEP,
                        help=f"sample every N-th minor (default {LADDER_STEP}, 1 is all)")
    parser.add_argument("--vendor", action="store_true",
                        help="write the proved icons into vendor/lucide-icons/")
    parser.add_argument("--check", action="store_true",
                        help="verify a vendored tree against its own record")
    args = parser.parse_args()

    if not REFERENCE.is_dir():
        print(f"missing reference icons: {REFERENCE}", file=sys.stderr)
        return 2

    releases = ladder(args.step)
    if args.check and not args.prove and not args.vendor:
        # A check reads the record rather than re-deriving it, so it needs no
        # network and no cache: the point of the record is to be checkable on a
        # machine that has neither.
        return check()

    found = prove(releases, args.icon)

    proved = sum(1 for row in found.values() if row["release"])
    refused = sum(1 for row in found.values() if row["reason"] == "refused by the reader")
    if args.icon:
        row = next(iter(found.values()))
        where = f"{row['release']} as `{row['lucide']}`" if row["release"] else row["reason"]
        print(f"{args.icon}: {where}")
    print()
    print(f"icons: {len(found)}")
    print(f"proved against a sampled Lucide release: {proved}")
    print(f"left in the reference tree: {len(found) - proved - refused}"
          f" (plus {refused} the reader refuses, which the build does not draw)")
    print()
    by_release: dict[str, int] = {}
    for row in found.values():
        if row["release"]:
            by_release[row["release"]] = by_release.get(row["release"], 0) + 1
    print(f"releases named: {len(by_release)} across the {len(releases)} sampled")
    for release, count in sorted(by_release.items(), key=lambda pair: key(pair[0]))[:12]:
        print(f"  {release}: {count}")

    if args.json:
        args.json.parent.mkdir(parents=True, exist_ok=True)
        args.json.write_text(json.dumps(found, indent=2, sort_keys=True), encoding="utf-8")
        print(f"\nwrote {args.json}")

    if args.vendor:
        vendor(found, releases)
    return 0


if __name__ == "__main__":
    sys.exit(main())
