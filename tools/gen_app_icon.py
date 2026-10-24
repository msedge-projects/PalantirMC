#!/usr/bin/env python3
"""Write the Windows icon files from the brand art.

    python tools/gen_app_icon.py            # write both .ico files
    python tools/gen_app_icon.py --check    # fail if what is on disk is not these bytes
    python tools/gen_app_icon.py --report   # the sizes and bytes of each entry

Two files, one set of bytes:

* `crates/palantir-desktop/assets/brand/app.ico` -- the icon `build.rs` hands to
  the resource compiler, so Explorer, the taskbar and the Alt-Tab list show the
  mark rather than the generic executable icon.
* `installer/sift.ico` -- the same icon for the installer, which reads its icon
  from a path beside its own script and therefore cannot share the file.

## Why the container is written here rather than by Pillow

NSIS refuses an ICO whose entries are PNG-compressed, and Pillow's ICO writer
chooses PNG for the largest sizes. Writing the container out -- seven
BITMAPINFOHEADER entries with 32-bit BGRA pixels and an all-zero AND mask -- is
about forty lines, is what both NSIS and Windows read, and makes `--check` a byte
comparison of a file whose bytes do not depend on a library's version.

## Where the pixels come from

`tools/gen_brand.py`'s recipe, imported rather than re-derived: the source art's
black backdrop becomes alpha, the colour is divided back out of it, the mark's ink
box is cropped to a square at 91%, and each size is one Lanczos pass from that
crop in premultiplied space. So the icon at 16 pixels and the hero logo at 512 are
the same drawing at different sizes, not two derivations that could drift.

`--check` is meaningful on the machine that wrote the file: the pixels come from
Pillow's Lanczos, and two Pillow versions can resample a fractional edge
differently by a least-significant bit. That is why CI does not run this -- the
Rust gate in `brand.rs` holds the container's shape at every size, which is the
part a resource compiler and Explorer actually depend on, and this script is the
command a person runs after touching the art.
"""

from __future__ import annotations

import argparse
import struct
import sys
from pathlib import Path

import numpy as np

from gen_brand import SOURCE, crop, isolated, resample

ROOT = Path(__file__).resolve().parent.parent
BRAND = ROOT / "crates" / "palantir-desktop" / "assets" / "brand"
OUTPUTS = [BRAND / "app.ico", ROOT / "installer" / "sift.ico"]

# The sizes Windows asks for: 16 and 32 in the shell's lists, 24 and 48 on the
# desktop, 64 for the Alt-Tab preview, and 128/256 for the large-icon views and
# Explorer's extra-large setting. 256 is the one size stored with a zero width
# byte, which is the format's way of saying 256.
SIZES = [16, 24, 32, 48, 64, 128, 256]


def entry_pixels(rgba: np.ndarray) -> bytes:
    """One icon entry's XOR bitmap: BGRA, bottom-up, as the format stores it."""
    rows = [rgba[row][:, [2, 1, 0, 3]].tobytes() for row in range(rgba.shape[0] - 1, -1, -1)]
    return b"".join(rows)


def entry_mask(side: int) -> bytes:
    """The AND mask, all zero: 32-bit entries carry their alpha in the XOR bitmap.

    The mask still has to be there and still has to be padded to four bytes a row,
    because the entry's height covers both bitmaps and a reader sizes the second
    from the first.
    """
    stride = ((side + 31) // 32) * 4
    return bytes(stride * side)


def entry(rgba: np.ndarray) -> bytes:
    """One ICONDIRENTRY's image: BITMAPINFOHEADER, XOR bitmap, AND mask."""
    side = rgba.shape[0]
    bitmap = entry_pixels(rgba)
    mask = entry_mask(side)
    header = struct.pack(
        "<IiiHHIIiiII",
        40,  # biSize
        side,  # biWidth
        side * 2,  # biHeight: the XOR bitmap and the AND mask together
        1,  # biPlanes
        32,  # biBitCount
        0,  # biCompression: BI_RGB
        len(bitmap) + len(mask),  # biSizeImage
        0, 0, 0, 0,  # resolution and palette, all unused at 32 bits
    )
    return header + bitmap + mask


def build() -> bytes:
    """The whole ICO file, byte for byte."""
    square = crop(isolated(SOURCE))
    images = [entry(resample(square, side)) for side in SIZES]
    directory = struct.pack("<HHH", 0, 1, len(images))
    offset = len(directory) + 16 * len(images)
    entries = b""
    for side, image in zip(SIZES, images):
        entries += struct.pack(
            "<BBBBHHII",
            side if side < 256 else 0,  # width, zero meaning 256
            side if side < 256 else 0,  # height
            0,  # palette colours
            0,  # reserved
            1,  # colour planes
            32,  # bits per pixel
            len(image),
            offset,
        )
        offset += len(image)
    return directory + entries + b"".join(images)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--check", action="store_true",
                        help="fail if the files on disk are not what this would write")
    parser.add_argument("--report", action="store_true",
                        help="print each size's byte count")
    args = parser.parse_args()

    if not SOURCE.is_file():
        print(f"missing brand art: {SOURCE}", file=sys.stderr)
        return 2

    blob = build()
    if args.report:
        square = crop(isolated(SOURCE))
        print(f"source   {SOURCE.name}")
        print(f"crop     {square.shape[1]}x{square.shape[0]}")
        for side in SIZES:
            print(f"entry    {side:>3}x{side:<3} {len(entry(resample(square, side))):>7} bytes")
        print(f"ico      {len(blob)} bytes, {len(SIZES)} entries, BMP (not PNG)")

    if args.check:
        stale = [path for path in OUTPUTS if not path.is_file() or path.read_bytes() != blob]
        if stale:
            for path in stale:
                print(f"{path.relative_to(ROOT)} is not what this tool writes", file=sys.stderr)
            print("Run `python tools/gen_app_icon.py` and commit the result.", file=sys.stderr)
            return 1
        print(f"{len(OUTPUTS)} icon files are current ({len(blob)} bytes each)")
        return 0

    for path in OUTPUTS:
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(blob)
        print(f"wrote {path.relative_to(ROOT)} ({len(blob)} bytes)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
