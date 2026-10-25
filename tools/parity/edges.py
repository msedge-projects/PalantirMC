#!/usr/bin/env python3
"""Report where a horizontal row of a capture changes colour, to the pixel.

Eyeballing a 1500 px composite cannot tell a 6 px inset from a 10 px one, and the parity
requirement is +/-2 px. This walks one row and prints every x where the colour moves by
more than `--threshold` in any channel, which is exactly where a panel, field or rule
starts and stops.

Usage:
    python3 tools/parity/edges.py --png out/discover-idle-modrinth.png --row 100
    python3 tools/parity/edges.py --png a.png b.png --row 100 --threshold 6

It shells out to ImageMagick (`convert ... txt:-`) rather than decoding PNG itself: the
VM has ImageMagick, and a hand-rolled decoder in a measurement tool is a source of
wrong numbers, not a virtue.
"""

from __future__ import annotations

import argparse
import subprocess
import sys


def row_pixels(png: str, row: int) -> list[tuple[int, int, int, int]]:
    """Every pixel of `row`, as (x, r, g, b)."""
    result = subprocess.run(
        ["convert", png, "-crop", f"+0+{row}", "+repage", "txt:-"],
        capture_output=True,
        text=True,
        check=False,
        timeout=120,
    )
    if result.returncode != 0:
        print(f"edges: convert failed for {png}: {result.stderr.strip()}", file=sys.stderr)
        return []
    pixels = []
    for line in result.stdout.splitlines():
        if not line or line.startswith("#"):
            continue
        # `0,0: (39,41,46)  #27292E  srgb(39,41,46)`
        head, _, rest = line.partition(":")
        x_text, _, _ = head.partition(",")
        start = rest.find("(")
        end = rest.find(")", start)
        if start < 0 or end < 0:
            continue
        parts = rest[start + 1 : end].split(",")
        if len(parts) < 3:
            continue
        try:
            pixels.append((int(x_text), int(parts[0]), int(parts[1]), int(parts[2])))
        except ValueError:
            continue
    return pixels


def edges(pixels: list[tuple[int, int, int, int]], threshold: int) -> list[int]:
    """The x of every pixel that differs from its left neighbour by > threshold."""
    found = []
    for previous, current in zip(pixels, pixels[1:]):
        _, r0, g0, b0 = previous
        x, r1, g1, b1 = current
        if max(abs(r1 - r0), abs(g1 - g0), abs(b1 - b0)) > threshold:
            found.append(x)
    return found


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--png", nargs="+", required=True)
    parser.add_argument("--row", type=int, required=True)
    parser.add_argument("--threshold", type=int, default=6,
                        help="per-channel change that counts as an edge (default 6)")
    parser.add_argument("--limit", type=int, default=40,
                        help="stop after this many edges (default 40)")
    args = parser.parse_args()

    for png in args.png:
        pixels = row_pixels(png, args.row)
        if not pixels:
            return 2
        found = edges(pixels, args.threshold)
        print(f"{png} row {args.row}: {len(found)} edges")
        for x in found[: args.limit]:
            print(f"  x={x}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
