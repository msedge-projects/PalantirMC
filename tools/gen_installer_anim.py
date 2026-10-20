#!/usr/bin/env python3
"""Draw the installer's animated mark and write it as an AVI.

The installer shows a rotating mark while it copies files. The reference's own
animation is two broken rings turning in opposite directions at different speeds
(4s one way, 6s the other, in `AnimatedLogo.vue`), which reads as depth without
needing a renderer: two arcs at different rates, and the eye supplies the rest.

This draws *this launcher's* mark in that motion. The rings are generated from
the theme's accent and background rather than taken from any picture, so the
animation is the product's own art; only the idea of counter-rotating arcs is
borrowed, and an idea is not a picture.

Output is an uncompressed 24-bit AVI, because that is what Windows' animated
control (`SysAnimate32`) can play without a codec: the installer has to work on a
machine with nothing installed on it.

    python3 tools/gen_installer_anim.py            # write
    python3 tools/gen_installer_anim.py --check     # regenerate and compare

The AVI is committed. `--check` fails when the committed file is not what this
script draws, which is the same contract `tools/gen_icons.py` keeps.
"""

from __future__ import annotations

import math
import struct
import sys
from pathlib import Path

from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "installer" / "anim" / "sift.avi"

# 96px is the largest size that still looks sharp in a 164px-tall header at 100%
# scaling and stays under a megabyte as uncompressed frames. 24 frames at 40ms is
# a one-second loop: fast enough to read as motion, slow enough not to flicker.
SIZE = 96
FRAMES = 24
FRAME_MS = 40

# The theme's own two colours, so the installer cannot drift from the window.
BACKDROP = (11, 11, 12)  # BG  #0B0B0C
ACCENT = (255, 122, 26)  # ACCENT #FF7A1A


def frame(index: int) -> Image.Image:
    """One frame: two arcs, counter-rotating, and a still mark in the middle."""
    image = Image.new("RGB", (SIZE, SIZE), BACKDROP)
    draw = ImageDraw.Draw(image)
    centre = SIZE / 2
    # The outer ring makes one turn per loop; the inner one makes one and a half
    # the other way, which is the 4s/6s ratio the reference's two rings turn at.
    outer = (index / FRAMES) * 360.0
    inner = -(index / FRAMES) * 540.0
    width = max(2, SIZE // 24)

    def arc(radius: float, start: float, sweep: float, colour, w: int) -> None:
        box = (centre - radius, centre - radius, centre + radius, centre + radius)
        draw.arc(box, start=start, end=start + sweep, fill=colour, width=w)

    # Two 100-degree bites out of each ring, so the mark reads as broken.
    arc(SIZE * 0.44, outer, 100, ACCENT, width)
    arc(SIZE * 0.44, outer + 180, 100, ACCENT, width)
    arc(SIZE * 0.30, inner, 90, ACCENT, width)
    arc(SIZE * 0.30, inner + 180, 90, ACCENT, width)
    # A dot at the centre: the still point the rings turn around.
    radius = SIZE * 0.045
    draw.ellipse(
        (centre - radius, centre - radius, centre + radius, centre + radius),
        fill=ACCENT,
    )
    return image


def dib(image: Image.Image) -> bytes:
    """One frame as a bottom-up 24-bit DIB, which is what an AVI frame holds."""
    header = struct.pack(
        "<IiiHHIIiiII",
        40,  # biSize
        SIZE,  # biWidth
        SIZE,  # biHeight (positive: bottom-up rows)
        1,  # biPlanes
        24,  # biBitCount
        0,  # biCompression: BI_RGB
        SIZE * SIZE * 3,  # biSizeImage
        0,
        0,
        0,
        0,
    )
    pixels = image.tobytes()  # top-down, RGB
    rows = [pixels[offset : offset + SIZE * 3] for offset in range(0, len(pixels), SIZE * 3)]
    body = b"".join(
        bytes(bgr for i in range(0, len(row), 3) for bgr in (row[i + 2], row[i + 1], row[i]))
        for row in reversed(rows)
    )
    return header + body


def chunk(tag: bytes, payload: bytes) -> bytes:
    """One RIFF chunk, padded to an even length."""
    pad = b"\x00" if len(payload) % 2 else b""
    return tag + struct.pack("<I", len(payload)) + payload + pad


def avi() -> bytes:
    """Assemble the container: header list, one stream, frames, index."""
    frame_bytes = [dib(frame(index)) for index in range(FRAMES)]
    frame_count = len(frame_bytes)

    avih = struct.pack(
        "<IIIIIIIIII4I",
        FRAME_MS * 1000,  # dwMicroSecPerFrame
        frame_count * FRAME_MS * 1000,  # dwMaxBytesPerSec (unused by players)
        0,  # dwPaddingGranularity
        0x10,  # dwFlags: AVIF_HASINDEX
        frame_count,
        0,
        1,  # dwStreams
        len(frame_bytes[0]),
        SIZE,
        SIZE,
        0,
        0,
        0,
        0,
    )
    strh = struct.pack(
        "<4s4sIHHIIIIIIIIhhhh",
        b"vids",
        b"DIB ",
        0,  # dwFlags
        0,  # wPriority
        0,  # wLanguage
        0,  # dwInitialFrames
        FRAME_MS,  # dwScale: rate/scale = 1000/40 = 25 fps, one frame per 40ms
        1000,  # dwRate
        0,  # dwStart
        frame_count,
        len(frame_bytes[0]),
        0xFFFFFFFF,  # dwQuality
        len(frame_bytes[0]),  # dwSampleSize
        0,
        0,
        SIZE,
        SIZE,
    )
    strf = struct.pack("<IiiHHIIiiII", 40, SIZE, SIZE, 1, 24, 0, SIZE * SIZE * 3, 0, 0, 0, 0)
    stream_list = chunk(
        b"LIST", b"strl" + chunk(b"strh", strh) + chunk(b"strf", strf)
    )
    hdrl = chunk(b"LIST", b"hdrl" + chunk(b"avih", avih) + stream_list)

    movi_payload = b"movi"
    index = b""
    offset = 4
    for data in frame_bytes:
        movi_payload += chunk(b"00db", data)
        index += struct.pack("<4sIII", b"00db", 0x10, offset, len(data))
        offset += 8 + len(data) + (len(data) % 2)
    movi = chunk(b"LIST", movi_payload)

    body = b"AVI " + hdrl + movi + chunk(b"idx1", index)
    return b"RIFF" + struct.pack("<I", len(body)) + body


def main(argv: list[str]) -> int:
    data = avi()
    if "--check" in argv:
        if not OUT.exists():
            print(f"{OUT.relative_to(ROOT)} is missing: run this script without --check")
            return 1
        if OUT.read_bytes() != data:
            print(f"{OUT.relative_to(ROOT)} is not what this script draws")
            return 1
        print(f"{OUT.relative_to(ROOT)}: {len(data)} bytes, up to date")
        return 0
    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_bytes(data)
    print(f"wrote {OUT.relative_to(ROOT)}: {len(data)} bytes, {FRAMES} frames")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
