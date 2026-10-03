#!/usr/bin/env python3
"""Fetch the reference's own Inter faces and unwrap them into plain sfnt.

Modrinth sets its whole interface in Inter (`packages/assets/styles/inter.scss`,
weights 400/500/600/700/800) and publishes the exact files it loads:

    https://cdn.modrinth.com/fonts/inter/Inter-{Regular,Medium,SemiBold,
    Bold,ExtraBold}.woff2?v=3.19

Those are the files, so they are what this ships. Two things stand between them
and a launcher:

  * **woff2 is unreadable here.** cosmic-text 0.10 (what iced 0.12 renders
    with) parses sfnt only -- TrueType/OpenType -- so the `.woff2` the
    stylesheet names first cannot be used at all.
  * **The `.woff` variants are, and they need no decompressor.** `inter.scss`
    lists a plain WOFF as each weight's second `src`, the CDN serves it (HTTP
    200, `font/woff`), and a WOFF is a container, not a compression: its tables
    are the sfnt's own bytes, deflated at most one at a time. Unwrapping it is
    a table-directory re-read, which is what `unwrap_woff` below does with
    nothing but `zlib`.

So no brotli and no fontTools are needed, which is also why this file has no
third-party dependency at all: the previous version shelled out to
`pyftsubset`, and the machine that builds this tree has no pip.

One thing this deliberately gives up. The old pipeline subset the upstream
release down to 505 codepoints (~292 KB for all five) and needed fontTools to
do it. These are the whole faces: 2505 codepoints, ~1.53 MB for all five. The
1.2 MB is the price of matching the reference's coverage rather than our guess
at it -- the CDN's build is not a subset, it is upstream's face whole (2505
codepoints and 2548 glyphs, the same counts as `Inter Desktop/*.otf` in the
official v3.19 archive), and the reference renders Cyrillic and Greek mod
descriptions out of it. A subset would have to guess which of those to keep.

Usage:
    python tools/make_fonts.py [--out DIR]
"""

from __future__ import annotations

import argparse
import pathlib
import struct
import sys
import urllib.request
import zlib

CDN = "https://cdn.modrinth.com/fonts/inter"
# The cache-buster `inter.scss` pins. It is not the font's own version string --
# see `VERSION_STRING` below.
CDN_QUERY = "v=3.19"

# The version Inter 3.19 puts in its own `name` table, ID 5. Inter zero-pads the
# minor component to three digits, so the 3.19 release writes "3.019"; the same
# string with the same git hash is in the upstream v3.19 archive's own
# `Inter Desktop/*.otf` and in every file the CDN serves. Reading it as "3.019,
# not 3.19" is what once had this tree's faces believed to be the wrong release.
VERSION_STRING = "Version 3.019;git-0a5106e0b"
# The typographic family (ID 16) is what fontdb resolves `theme::FAMILY`
# against; ID 1 carries the per-weight name ("Inter Medium").
FAMILY_STRING = "Inter"
UPM = 2816

# Font weight -> the face's filename stem on the CDN.
WEIGHTS = {
    400: "Regular",
    500: "Medium",
    600: "SemiBold",
    700: "Bold",
    800: "ExtraBold",
}


def download(face: str, dest: pathlib.Path) -> bytes:
    if dest.exists():
        print(f"using existing {dest}")
        return dest.read_bytes()
    url = f"{CDN}/Inter-{face}.woff?{CDN_QUERY}"
    print(f"GET {url}")
    dest.parent.mkdir(parents=True, exist_ok=True)
    with urllib.request.urlopen(url) as response:
        data = response.read()
    dest.write_bytes(data)
    return data


def _checksum(data: bytes) -> int:
    data += b"\0" * (-len(data) % 4)
    total = 0
    for i in range(0, len(data), 4):
        total = (total + struct.unpack(">I", data[i:i + 4])[0]) & 0xFFFFFFFF
    return total


def unwrap_woff(data: bytes) -> bytes:
    """A WOFF container's tables, reassembled as the sfnt they came from."""
    if data[:4] != b"wOFF":
        raise ValueError(f"not a WOFF container: {data[:4]!r}")
    flavor, _length, num_tables, _reserved = struct.unpack(">4sIHH", data[4:16])
    tables = []
    for i in range(num_tables):
        tag, offset, comp_len, orig_len, _sum = struct.unpack(
            ">4sIIII", data[44 + i * 20: 64 + i * 20]
        )
        raw = data[offset:offset + comp_len]
        if comp_len < orig_len:
            raw = zlib.decompress(raw)
        if len(raw) != orig_len:
            raise ValueError(
                f"table {tag!r} is {len(raw)} bytes, directory says {orig_len}"
            )
        tables.append((tag, raw))

    entry_selector = max(num_tables.bit_length() - 1, 0)
    search_range = (1 << entry_selector) * 16
    header = flavor + struct.pack(
        ">HHHH", num_tables, search_range, entry_selector,
        num_tables * 16 - search_range,
    )
    body = b""
    offset = 12 + num_tables * 16
    directory = b""
    for tag, raw in tables:
        directory += struct.pack(">4sIII", tag, _checksum(raw), offset, len(raw))
        pad = -len(raw) % 4
        body += raw + b"\0" * pad
        offset += len(raw) + pad

    font = bytearray(header + directory + body)

    # head.checkSumAdjustment is defined as 0xB1B0AFBA minus the whole file's
    # checksum, and it is the one field that depends on where the tables were
    # put. Nothing that reads the font here uses it, but a file whose checksum
    # does not close is a file the next tool will complain about.
    head_rec = next(
        12 + i * 16 for i in range(num_tables)
        if font[12 + i * 16: 16 + i * 16] == b"head"
    )
    head_off = struct.unpack(">I", font[head_rec + 8: head_rec + 12])[0]
    struct.pack_into(">I", font, head_off + 8, 0)
    adjustment = (0xB1B0AFBA - _checksum(bytes(font))) & 0xFFFFFFFF
    struct.pack_into(">I", font, head_off + 8, adjustment)
    return bytes(font)


def names(font: bytes) -> dict[int, str]:
    """name ID -> text, from the platform-3 (Windows) records."""
    num_tables = struct.unpack(">H", font[4:6])[0]
    base = offset = 0
    for i in range(num_tables):
        tag, _sum, off, length = struct.unpack(
            ">4sIII", font[12 + i * 16: 28 + i * 16]
        )
        if tag == b"name":
            base, offset = off, length
            break
    if not base:
        return {}
    count, str_off = struct.unpack(">HH", font[base + 2: base + 6])
    out: dict[int, str] = {}
    for i in range(count):
        pid, _eid, _lid, nid, ln, at = struct.unpack(
            ">HHHHHH", font[base + 6 + i * 12: base + 18 + i * 12]
        )
        if pid != 3:
            continue
        out.setdefault(nid, font[base + str_off + at: base + str_off + at + ln]
                       .decode("utf-16-be", errors="replace"))
    return out


def verify(font: bytes, face: str) -> None:
    """Refuse to ship a face that is not the one the reference loads.

    A wrong decompression produces a file that looks fine and renders nothing,
    so every claim this file makes about a face is checked against the bytes
    that were just written rather than against what was asked for.
    """
    got = names(font)
    if got.get(5) != VERSION_STRING:
        raise SystemExit(
            f"Inter-{face}: name ID 5 is {got.get(5)!r}, expected {VERSION_STRING!r}"
        )
    # The family a face registers under is its typographic family (ID 16) when
    # it has one and its plain family (ID 1) when it does not -- upstream only
    # writes ID 16 for the weights whose ID 1 is not already "Inter", so
    # Regular and Bold carry none. `theme::FAMILY` resolves against exactly
    # this, and all five faces must land on the same one or weight selection
    # finds fewer than five.
    family = got.get(16) or got.get(1)
    if family != FAMILY_STRING:
        raise SystemExit(
            f"Inter-{face}: family is {family!r}, expected {FAMILY_STRING!r}"
        )
    if got.get(6) != f"Inter-{face}":
        raise SystemExit(
            f"Inter-{face}: PostScript name is {got.get(6)!r}"
        )
    num_tables = struct.unpack(">H", font[4:6])[0]
    for i in range(num_tables):
        tag, _sum, off, length = struct.unpack(
            ">4sIII", font[12 + i * 16: 28 + i * 16]
        )
        if tag == b"head":
            upem = struct.unpack(">H", font[off + 18: off + 20])[0]
            if upem != UPM:
                raise SystemExit(f"Inter-{face}: unitsPerEm is {upem}, expected {UPM}")
    for needed in (b"glyf", b"loca", b"cmap", b"hmtx", b"head"):
        if needed not in [font[12 + i * 16: 16 + i * 16] for i in range(num_tables)]:
            raise SystemExit(f"Inter-{face}: no {needed.decode()} table")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--out",
        type=pathlib.Path,
        default=pathlib.Path("crates/palantir-desktop/assets/fonts"),
    )
    parser.add_argument(
        "--cache",
        type=pathlib.Path,
        default=pathlib.Path("target/font-cache"),
        help="where the downloaded .woff files are kept between runs",
    )
    args = parser.parse_args()

    args.out.mkdir(parents=True, exist_ok=True)
    total = 0
    for weight, face in WEIGHTS.items():
        woff = download(face, args.cache / f"Inter-{face}.woff")
        font = unwrap_woff(woff)
        verify(font, face)
        target = args.out / f"Inter-{weight}.ttf"
        target.write_bytes(font)
        total += len(font)
        print(f"  {face:18} {len(woff) / 1024:6.0f} KB woff -> "
              f"{len(font) / 1024:6.0f} KB  {target.name}")
    print(f"\n{len(WEIGHTS)} faces, {total / 1024:.0f} KB total in {args.out}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
