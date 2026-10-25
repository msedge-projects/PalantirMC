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
    """Every pixel of `row`, as (x, r, g, b).

    Raw RGB bytes rather than ImageMagick's text output: the text form has to be parsed,
    and the first version of this tool parsed it wrong -- it reported 102,926 "edges" in a
    1,440-pixel row, which is a decoder bug stating its own result. A byte stream of
    `width * 3` bytes cannot be misread.
    """
    size = subprocess.run(
        ["identify", "-format", "%w", png], capture_output=True, text=True, check=False,
        timeout=60,
    )
    try:
        width = int(size.stdout.strip())
    except ValueError:
        print(f"edges: cannot read width of {png}", file=sys.stderr)
        return []
    result = subprocess.run(
        ["convert", png, "-crop", f"{width}x1+0+{row}", "+repage", "rgb:-"],
        capture_output=True, check=False, timeout=120,
    )
    if result.returncode != 0:
        print(f"edges: convert failed for {png}", file=sys.stderr)
        return []
    raw = result.stdout
    return [
        (index // 3, raw[index], raw[index + 1], raw[index + 2])
        for index in range(0, min(len(raw), width * 3) - 2, 3)
    ]


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
