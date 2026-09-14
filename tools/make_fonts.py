#!/usr/bin/env python3
"""Subset Inter into the weights PalantirMC ships.

Modrinth sets its whole interface in Inter (`packages/assets/styles/inter.scss`,
weights 400/500/600/700/800). Reproducing that means shipping the font, and the
full faces are far too large for a launcher: the unhinted desktop OTFs are
~260 KB each and the hinted Windows TTFs ~700 KB, so five weights would add
1.3-3.5 MB to an 8 MB executable.

Why these files and not the website's:
  * Modrinth serves Inter as woff2 from its CDN. cosmic-text 0.10 (what iced
    0.12 renders with) reads sfnt only -- TrueType/OpenType -- so a woff2
    cannot be used at all.
  * It has no variable-font support either (no `fvar` handling anywhere in the
    crate), so `Inter-V.ttf` would render every weight identically. One file
    per weight is the only thing that works.
  * The desktop OTFs are used rather than the hinted TTFs because swash, which
    does the rasterising, applies its own scaling and ignores GDI hinting --
    paying 2.7x the bytes for hinting nothing reads would be waste.

The subset keeps Latin plus the punctuation, arrows and symbols a launcher
actually draws. Anything outside it (a CJK instance name, an emoji in a mod
title) falls back to a system font, which cosmic-text does per glyph on its
own, so this is a size decision and not a coverage one.

Usage:
    python tools/make_fonts.py [--zip Inter-3.19.zip] [--out DIR]

Without --zip it downloads the official release. Requires fonttools:

    pip install fonttools
"""

from __future__ import annotations

import argparse
import io
import os
import pathlib
import shutil
import subprocess
import sys
import urllib.request
import zipfile

RELEASE = "https://github.com/rsms/inter/releases/download/v3.19/Inter-3.19.zip"
ZIP_NAME = "Inter-3.19.zip"

# Font weight -> the face's name inside the archive.
WEIGHTS = {
    400: "Inter-Regular",
    500: "Inter-Medium",
    600: "Inter-SemiBold",
    700: "Inter-Bold",
    800: "Inter-ExtraBold",
}

# Latin, punctuation, currency, arrows and the handful of symbols the chrome
# uses. Deliberately not `--unicodes=*`: that is the 260 KB face again.
UNICODES = ",".join(
    [
        "U+0020-007E",  # Basic Latin
        "U+00A0-00FF",  # Latin-1 Supplement
        "U+0100-017F",  # Latin Extended-A
        "U+2000-206F",  # General Punctuation: en/em dash, ellipsis, quotes
        "U+20A0-20BF",  # Currency
        "U+2190-21FF",  # Arrows
        "U+2202,U+2206,U+220F,U+2211,U+2212,U+2215,U+2219,U+221A,U+221E,U+222B",
        "U+2248,U+2260,U+2261,U+2264,U+2265,U+25CA",
        "U+25A0-25CF",  # Geometric shapes: squares, circles, triangles
        "U+2600-26FF",  # Misc symbols: check marks, stars
        "U+2713,U+2714,U+2715,U+2717,U+2718,U+274C,U+274E",  # tick / cross
    ]
)


def download(dest: pathlib.Path) -> pathlib.Path:
    if dest.exists():
        print(f"using existing {dest}")
        return dest
    print(f"downloading {RELEASE}")
    dest.parent.mkdir(parents=True, exist_ok=True)
    with urllib.request.urlopen(RELEASE) as response, open(dest, "wb") as fh:
        shutil.copyfileobj(response, fh)
    return dest


def subset(zip_path: pathlib.Path, out_dir: pathlib.Path) -> int:
    out_dir.mkdir(parents=True, exist_ok=True)
    written = 0
    with zipfile.ZipFile(zip_path) as archive:
        for weight, face in WEIGHTS.items():
            member = f"Inter Desktop/{face}.otf"
            target = out_dir / f"Inter-{weight}.otf"
            source = archive.read(member)
            # pyftsubset wants a path, so stage the face in a temp file next to
            # the output and clean it up; keeps the tool runnable from anywhere.
            staged = out_dir / f".{face}.otf.source"
            staged.write_bytes(source)
            try:
                subprocess.run(
                    [
                        sys.executable,
                        "-m",
                        "fontTools.subset",
                        str(staged),
                        f"--unicodes={UNICODES}",
                        # Kerning, standard ligatures and contextual alternates
                        # are what a UI draws; keeping every feature costs
                        # ~30 KB per face in GSUB/GPOS tables nothing reads.
                        "--layout-features=kern,liga,calt",
                        # Names are kept, including the copyright and licence
                        # records, because the OFL asks for them to travel with
                        # the font.
                        "--name-IDs=*",
                        "--notdef-outline",
                        "--recommended-glyphs",
                        f"--output-file={target}",
                    ],
                    check=True,
                    capture_output=True,
                )
            except subprocess.CalledProcessError as exc:
                print(exc.stderr.decode(errors="replace"), file=sys.stderr)
                raise
            finally:
                staged.unlink(missing_ok=True)

            before = len(source) / 1024
            after = target.stat().st_size / 1024
            print(f"  {face:18} {before:7.0f} KB -> {after:6.1f} KB  {target.name}")
            written += 1
    return written


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--zip", type=pathlib.Path, default=pathlib.Path(ZIP_NAME))
    parser.add_argument(
        "--out",
        type=pathlib.Path,
        default=pathlib.Path("crates/palantir-desktop/assets/fonts"),
    )
    args = parser.parse_args()

    try:
        import fontTools  # noqa: F401
    except ImportError:
        print("fonttools is required: pip install fonttools", file=sys.stderr)
        return 2

    written = subset(download(args.zip).resolve(), args.out)
    total = sum(f.stat().st_size for f in args.out.glob("*.otf")) / 1024
    print(f"\n{written} faces, {total:.0f} KB total in {args.out}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
