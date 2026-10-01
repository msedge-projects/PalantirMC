#!/usr/bin/env python3
"""Derive the launcher's hero logo and window icon from the brand source art.

The two files the GUI embeds are crops of `assets/brand/palantirmc.png`, and this
is what takes the crop, so the derivation is a command rather than a paragraph
somebody has to re-invent on the next logo:

    python tools/gen_brand.py            # write logo512.png and icon256.png
    python tools/gen_brand.py --check    # fail if the files on disk are not these bytes
    python tools/gen_brand.py --report   # the measurements the crop was taken from

## Why the source art is not embedded as it stands

The art is drawn on a black tile, and `include_bytes!`-ing it would put that tile
in the window: a black square beside the welcome copy and in the taskbar, on a UI
whose own surfaces are dark but not black. So the backdrop becomes transparency,
which is the first thing this does.

## From black to alpha

Black is exactly zero, so a pixel's own brightest channel *is* its alpha: a
stroke of the mark at full strength has one channel at 255, and a pixel of the
glow around it is as opaque as its strongest channel says. Nothing has to be
guessed or keyed, and no colour that is genuinely dark inside the mark is cut
out, because a dark colour has a low brightest channel -- which is the same
measure as its opacity.

The colour is then divided back out of that alpha. Left as measured, the source
pixels are already premultiplied, and compositing premultiplied colour over a
dark surface darkens it twice: a glow pixel of `(60, 120, 30)` would draw as
`(28, 56, 14)`. Dividing it by its own alpha restores the colour the art has, so
the mark lands on the UI at the brightness it was drawn at, and the glow
*lightens* what is under it -- which is what a glow is for.

Pixels below 8/255 are cleared outright. That is not tidiness: it is the
resampler's ringing and the last of the glow, and kept, they turn into a grey
haze in the corners of the tile.

## The crop

The mark is taller than it is wide, so a square crop of the source would either
clip it or leave it small and off-centre. Instead the ink's own bounding box is
measured and a square is centred on it, sized so the mark's longest side covers
91% of it -- the proportion the first mark in this tree was cropped at, and the
one that leaves a margin that is even on all four sides. Centred matters twice
over: `brand::tests::window_icon_decodes_to_256_square` asserts the icon's ink is
centred in its square, and a crop that is not centred on the ink fails it.

## Resampling

Each output is resampled straight from the crop -- one Lanczos pass, not a chain
of them -- in premultiplied space, which is the only way alpha can be averaged
without dragging the mark's edge towards the black that is no longer there. The
floor is applied again afterwards, because resampling lifts it.
"""

from __future__ import annotations

import argparse
import math
import sys
from io import BytesIO
from pathlib import Path

import numpy as np
from PIL import Image

ROOT = Path(__file__).resolve().parent.parent
BRAND = ROOT / "crates" / "palantir-desktop" / "assets" / "brand"
SOURCE = BRAND / "palantirmc.png"

# The two embedded files, with the side each is cut at.
OUTPUTS = [(BRAND / "logo512.png", 512), (BRAND / "icon256.png", 256)]

# Below this alpha a pixel is the resampler's tail rather than the mark.
ALPHA_FLOOR = 8

# The mark's longest side, as a fraction of the square crop's side.
INK_SHARE = 0.91


def isolated(path: Path) -> np.ndarray:
    """The source art as RGBA: black backdrop to alpha, colour divided back out."""
    rgb = np.asarray(Image.open(path).convert("RGB"), dtype=np.uint16)
    alpha = rgb.max(axis=2)
    # Rounded division rather than truncation: `156 * 255 / 251` is 158.5 and the
    # art's own stroke colour, so rounding keeps it at 158 instead of darkening
    # every bright pixel by the same half step.
    colour = np.where(
        alpha[..., None] > 0,
        np.minimum(255, (rgb * 255 + alpha[..., None] // 2) // np.maximum(alpha[..., None], 1)),
        0,
    ).astype(np.uint8)
    rgba = np.dstack([colour, alpha.astype(np.uint8)])
    rgba[alpha < ALPHA_FLOOR] = 0
    return rgba


def ink_box(rgba: np.ndarray) -> tuple[int, int, int, int]:
    """`(left, top, right, bottom)` of everything at or above the alpha floor."""
    ys, xs = np.nonzero(rgba[..., 3] >= ALPHA_FLOOR)
    if not len(xs):
        raise SystemExit(f"{SOURCE.name}: no artwork above the alpha floor")
    return int(xs.min()), int(ys.min()), int(xs.max()), int(ys.max())


def crop(rgba: np.ndarray) -> np.ndarray:
    """The square around the mark's ink box that both outputs are cut from."""
    height, width = rgba.shape[:2]
    left, top, right, bottom = ink_box(rgba)
    side = int(math.ceil(max(right - left + 1, bottom - top + 1) / INK_SHARE))
    side = min(side, width, height)
    # Centre on the ink, then slide back inside the source if that would hang off
    # an edge -- the mark is not centred in its own tile, and cannot be asked to
    # move, so the crop is what gives.
    x0 = int(round((left + right + 1) / 2 - side / 2))
    y0 = int(round((top + bottom + 1) / 2 - side / 2))
    x0 = max(0, min(x0, width - side))
    y0 = max(0, min(y0, height - side))
    return rgba[y0:y0 + side, x0:x0 + side]


def resample(rgba: np.ndarray, side: int) -> np.ndarray:
    """One Lanczos pass to `side` square, averaged as premultiplied colour.

    Premultiplied because the alternative averages a stroke's edge against the
    transparent black around it and draws a dark rim no pixel of the art has.
    Each plane goes through PIL as 8-bit, which is where the integer truncation
    `NOTES.md` measures comes from.
    """
    if rgba.shape[0] == side and rgba.shape[1] == side:
        return rgba.copy()
    alpha = rgba[..., 3]
    premultiplied = [
        (rgba[..., channel].astype(np.uint16) * alpha // 255).astype(np.uint8)
        for channel in range(3)
    ]
    planes = [
        np.asarray(Image.fromarray(plane).resize((side, side), Image.LANCZOS), dtype=np.uint16)
        for plane in premultiplied
    ]
    scaled_alpha = np.asarray(
        Image.fromarray(alpha).resize((side, side), Image.LANCZOS), dtype=np.uint16
    )
    out = np.zeros((side, side, 4), dtype=np.uint8)
    divisor = np.maximum(scaled_alpha, 1)
    for channel, plane in enumerate(planes):
        out[..., channel] = np.where(
            scaled_alpha > 0,
            np.minimum(255, (plane * 255 + divisor // 2) // divisor),
            0,
        ).astype(np.uint8)
    out[..., 3] = scaled_alpha
    out[scaled_alpha < ALPHA_FLOOR] = 0
    return out


def encoded(rgba: np.ndarray) -> bytes:
    """The PNG bytes for one output.

    Encoded through a buffer rather than straight to disk so `--check` can
    compare what this tool would write against what is committed without touching
    the tree when they differ.
    """
    buffer = BytesIO()
    Image.fromarray(rgba).save(buffer, format="PNG", optimize=True)
    return buffer.getvalue()


def build() -> tuple[list[tuple[Path, bytes]], np.ndarray]:
    """Every output as `(path, bytes)`, plus the crop they all come from."""
    rgba = isolated(SOURCE)
    square = crop(rgba)
    return [(path, encoded(resample(square, side))) for path, side in OUTPUTS], square


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--check", action="store_true",
                        help="fail if the files on disk are not what this would write")
    parser.add_argument("--report", action="store_true",
                        help="print the measurements the crop was taken from")
    args = parser.parse_args()

    if not SOURCE.is_file():
        print(f"missing brand art: {SOURCE}", file=sys.stderr)
        return 2

    outputs, square = build()
    if args.report:
        rgba = isolated(SOURCE)
        left, top, right, bottom = ink_box(rgba)
        print(f"source      {SOURCE.name}: {rgba.shape[1]}x{rgba.shape[0]}")
        print(f"ink box     {left},{top}..{right},{bottom} "
              f"({right - left + 1}x{bottom - top + 1}, "
              f"{100 * max(right - left + 1, bottom - top + 1) / rgba.shape[1]:.1f}% of the source)")
        print(f"crop        {square.shape[1]}x{square.shape[0]} "
              f"(ink covers {100 * max(right - left + 1, bottom - top + 1) / square.shape[1]:.1f}%)")
        for path, blob in outputs:
            plane = np.asarray(Image.open(BytesIO(blob)), dtype=np.uint8)
            alpha = plane[..., 3]
            opaque = int((alpha == 255).sum())
            print(f"{path.name:<13} {plane.shape[1]}x{plane.shape[0]} "
                  f"{len(blob)} bytes, clear {int((alpha == 0).sum())/(alpha.size):.1%}, "
                  f"opaque {opaque}, ink {int(alpha.astype(np.uint64).sum())}")

    if args.check:
        stale = [
            path for path, blob in outputs
            if not path.is_file() or path.read_bytes() != blob
        ]
        if stale:
            for path in stale:
                print(f"{path.relative_to(ROOT)} is not what this tool writes", file=sys.stderr)
            print("Run `python tools/gen_brand.py` and commit the result.", file=sys.stderr)
            return 1
        print(f"{len(outputs)} brand files are current")
        return 0

    for path, blob in outputs:
        path.write_bytes(blob)
        print(f"wrote {path.relative_to(ROOT)} ({len(blob)} bytes)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
