#!/usr/bin/env python3
"""Derive the launcher's brand assets from the Sift mark.

    python tools/gen_brand.py              # write the two PNGs and both .ico files
    python tools/gen_brand.py --check      # fail if the files on disk are not these bytes
    python tools/gen_brand.py --report     # the measurements, plus each icon entry
    python tools/gen_brand.py --self-test  # prove a change to the source's alpha changes these

Four files, one drawing:

* `assets/brand/logo512.png` (512x512 RGBA) -- the welcome hero and title mark.
* `assets/brand/icon256.png` (256x256 RGBA) -- the window and taskbar icon.
* `assets/brand/app.ico` -- the exe's icon, for `build.rs` and the resource compiler.
* `installer/sift.ico` -- the same bytes for the installer, which reads its icon
  from a path beside its own script and so cannot share the file.

## The alpha is the source's, not this tool's

The source, `assets/brand/sift-logo-source.png`, is drawn on a real
transparent background. The recipe this script carried before was written for a
mark that sat on a black tile: alpha was *derived* from each pixel's brightest
channel, because black is zero and so the brightest channel was the opacity.
Run that on art that already has alpha and it destroys the drawing -- a dark
edge inside the mark reads as nearly transparent, every solid dark stroke comes
back as a haze, and the old tile's silhouette is re-added as a halo. So nothing
here derives anything. The alpha channel is taken as given and used for two
things only: finding the mark, and deciding which pixels count as empty.

## The crop

The ink box is the bounding box of every pixel with alpha above 8 -- low enough
to keep the softest stroke the artist drew, high enough to ignore an encoder's
rounding noise. A square is centred on that box with the mark's longest side
covering 91% of it: the proportion the previous mark in this tree was cropped
at, kept so the hero tile and the taskbar are framed as they were. Centred
matters twice over -- `brand.rs`'s `window_icon_decodes_to_256_square` asserts
the ink is centred in its square -- and the square slides back inside the
source if centring would hang it off an edge, because the mark cannot be asked
to move.

## Resampling

One Lanczos pass per output size, straight from that square, in premultiplied
space: colour is multiplied by alpha before it is filtered and divided back out
after, so a stroke's edge averages against its own colour rather than against
the transparent black behind it -- which is where the dark fringe on
transparent art comes from. The division rounds, and alpha below 8 is cleared
again afterwards because the filter lifts the floor back up.

## The icon

Seven sizes -- 16, 24, 32, 48, 64, 128 and 256 -- each a classic BMP entry: a
40-byte BITMAPINFOHEADER, the pixels as 32-bit BGRA bottom-up, and an all-zero
AND mask padded to four bytes a row. PNG-compressed entries are smaller and
newer and are deliberately not used: NSIS refuses them, and `brand.rs`'s icon
test holds the entry arithmetic (`40 + w*h*4 + stride*h`), which is the part a
resource compiler and Explorer depend on. The container is assembled here
rather than handed to Pillow's ICO writer, because that writer resizes each
frame itself, through `thumbnail` in straight alpha -- the dark fringe again at
the sizes where it shows most -- and writes a plane count of zero.

## Which renderer

Pillow if it imports -- the fast path, and the one the committed files were
written with. Otherwise ImageMagick (`magick`, or `convert` on 6.x), which does
the same work with `-alpha premultiply` / `-alpha unpremultiply`. If neither is
installed the tool stops and names what to install rather than guessing.
ImageMagick is recognised by its `-version` banner, not by the executable's
name: Windows ships an unrelated `convert.exe`.

`--check` compares bytes, and the pixels are the resampler's: Pillow and
ImageMagick filter a fractional edge differently, and two Pillow versions can
differ by a least-significant bit. So it is the command a person runs after
touching the art, on the machine that wrote the files, which is how `GATES.md`
and `brand.rs` describe it. What holds in CI is the container's shape, in
`brand.rs`'s icon test, and that does not depend on an encoder.
"""

from __future__ import annotations

import argparse
import hashlib
import math
import re
import shutil
import struct
import subprocess
import sys
import tempfile
from io import BytesIO
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parent.parent
# The brand art lives at the repository root now. It used to sit inside the iced crate,
# which is why this path named it; that crate is deleted and the art belongs to the product.
BRAND = ROOT / "assets" / "brand"
SOURCE = BRAND / "sift-logo-source.png"

# The two embedded PNGs, with the side each is rendered at.
OUTPUTS = [(BRAND / "logo512.png", 512), (BRAND / "icon256.png", 256)]

# The exe's icon and the installer's copy of it: one file's bytes, two paths.
ICON = BRAND / "app.ico"
INSTALLER_ICON = ROOT / "installer" / "sift.ico"

# The sizes Windows asks for: 16 and 32 in the shell's lists, 24 and 48 on the
# desktop, 64 for the Alt-Tab preview, 128 and 256 for the large-icon views.
ICON_SIZES = [16, 24, 32, 48, 64, 128, 256]

# At or below this alpha a pixel is empty: the softest stroke the artist drew
# is above it, and an encoder's rounding noise is below it.
ALPHA_FLOOR = 8

# The mark's longest side, as a fraction of the square crop's side.
INK_SHARE = 0.91

PNG_MAGIC = b"\x89PNG\r\n\x1a\n"

# `straight = round(premultiplied * 255 / alpha)`, tabulated for every
# (alpha, channel) pair so the division is one lookup instead of a pass over
# the image deciding what to divide by.
_RECIPROCAL = bytes(
    min(255, (channel * 255 + alpha // 2) // alpha) if alpha else 0
    for alpha in range(256)
    for channel in range(256)
)


class BackendMissing(Exception):
    """Neither imaging path could be loaded."""


# --------------------------------------------------------------------------
# The recipe, which is the same whichever renderer carries it out
# --------------------------------------------------------------------------


def square_box(
    ink: tuple[int, int, int, int], size: tuple[int, int]
) -> tuple[int, int, int]:
    """`(left, top, side)` of the square every output is cut from.

    `ink` is `(left, top, right, bottom)`, right and bottom exclusive, the way
    `getbbox()` returns it, and `size` is the source's `(width, height)`.
    """
    left, top, right, bottom = ink
    width, height = right - left, bottom - top
    if width <= 0 or height <= 0:
        raise SystemExit(f"{SOURCE.name}: no artwork above alpha {ALPHA_FLOOR}")
    side = int(math.ceil(max(width, height) / INK_SHARE))
    side = min(side, size[0], size[1])
    x0 = round((left + right) / 2 - side / 2)
    y0 = round((top + bottom) / 2 - side / 2)
    x0 = max(0, min(x0, size[0] - side))
    y0 = max(0, min(y0, size[1] - side))
    return x0, y0, side


def compose(image_module: Any, planes: list[Any], alpha: Any) -> Any:
    """The RGBA raster: colour divided back out of alpha, then the floor applied.

    `planes` are the premultiplied R, G and B bands after the same resize as
    `alpha`. A pixel below the floor is cleared outright -- colour as well as
    alpha, so a compositor that ignores alpha at small sizes cannot paint the
    filter's ringing.
    """
    cover = alpha.tobytes()
    red, green, blue = (plane.tobytes() for plane in planes)
    out = bytearray(len(cover) * 4)
    for index, value in enumerate(cover):
        if value < ALPHA_FLOOR:
            continue
        at = index * 4
        row = value << 8
        out[at] = _RECIPROCAL[row + red[index]]
        out[at + 1] = _RECIPROCAL[row + green[index]]
        out[at + 2] = _RECIPROCAL[row + blue[index]]
        out[at + 3] = value
    return image_module.frombytes("RGBA", alpha.size, bytes(out))


def bgra_bottom_up(rgba: bytes, side: int) -> bytes:
    """The XOR bitmap of one icon entry: BGRA, the last row first.

    The format stores its rows the way a DIB does, which is upside down
    relative to every PNG in this tree. Swapping the red and blue planes and
    reversing the rows wholesale keeps this a handful of slice copies rather
    than a loop over a quarter of a million pixels.
    """
    swapped = bytearray(len(rgba))
    swapped[0::4] = rgba[2::4]
    swapped[1::4] = rgba[1::4]
    swapped[2::4] = rgba[0::4]
    swapped[3::4] = rgba[3::4]
    stride = side * 4
    out = bytearray(len(rgba))
    for row in range(side):
        start = (side - 1 - row) * stride
        out[row * stride:(row + 1) * stride] = swapped[start:start + stride]
    return bytes(out)


def icon_entry(side: int, rgba: bytes) -> bytes:
    """One ICONDIRENTRY's image: BITMAPINFOHEADER, XOR bitmap, AND mask."""
    if len(rgba) != side * side * 4:
        raise SystemExit(f"the {side}px icon frame is {len(rgba)} bytes, not {side * side * 4}")
    bitmap = bgra_bottom_up(rgba, side)
    # The mask is all zero because a 32-bit entry carries its alpha in the XOR
    # bitmap; it still has to be there, padded to four bytes a row, because the
    # entry's height covers both bitmaps and a reader sizes the second from the
    # first.
    stride = ((side + 31) // 32) * 4
    mask = bytes(stride * side)
    header = struct.pack(
        "<IiiHHIIiiII",
        40,  # biSize
        side,  # biWidth
        side * 2,  # biHeight: the XOR bitmap and the AND mask together
        1,  # biPlanes
        32,  # biBitCount
        0,  # biCompression: BI_RGB
        len(bitmap) + len(mask),  # biSizeImage
        0, 0, 0, 0,  # resolution and palette, unused at 32 bits
    )
    entry = header + bitmap + mask
    # The same sum `brand.rs`'s icon test makes, checked before it is written.
    expected = 40 + side * side * 4 + stride * side
    if len(entry) != expected:
        raise SystemExit(f"the {side}px icon entry is {len(entry)} bytes, not {expected}")
    return entry


def ico_file(frames: list[tuple[int, bytes]]) -> bytes:
    """The ICO: ICONDIR, one ICONDIRENTRY per size, then each size's entry."""
    expected = sorted(ICON_SIZES)
    if [side for side, _ in frames] != expected:
        raise SystemExit(f"icon frames are {[side for side, _ in frames]}, not {expected}")
    entries = [(side, icon_entry(side, rgba)) for side, rgba in frames]
    header = struct.pack("<HHH", 0, 1, len(entries))
    offset = len(header) + 16 * len(entries)
    directory = b""
    for side, entry in entries:
        stored = side if side < 256 else 0  # zero is the format's way of saying 256
        directory += struct.pack(
            "<BBBBHHII",
            stored,  # width
            stored,  # height
            0,  # palette colours: none, this is true colour
            0,  # reserved
            1,  # colour planes
            32,  # bits per pixel: alpha is in the pixels
            len(entry),
            offset,
        )
        offset += len(entry)
    return header + directory + b"".join(entry for _, entry in entries)


def build(
    backend: Any, source: Path
) -> tuple[list[tuple[Path, int | None, bytes]], dict[str, Any]]:
    """Every output as `(path, side or None, bytes)`, plus the measurements.

    Built entirely in memory (or in the backend's scratch directory) so
    `--check` can compare against the tree without touching it.
    """
    art = backend.load(source)
    size = backend.size(art)
    ink = backend.ink_box(art)
    x0, y0, side = square_box(ink, size)
    square = backend.crop(art, (x0, y0, side))
    premultiplied = backend.premultiplied(square)

    # One render per side, kept as a handle: `icon256.png` and the icon's 256
    # entry are then the same pixels rather than two chances to disagree.
    handles: dict[int, Any] = {}

    def render(side_px: int) -> Any:
        if side_px not in handles:
            handles[side_px] = backend.straight(premultiplied, side_px)
        return handles[side_px]

    outputs: list[tuple[Path, int | None, bytes]] = [
        (path, side_px, backend.png(render(side_px))) for path, side_px in OUTPUTS
    ]
    icon = ico_file([(side_px, backend.rgba(render(side_px), side_px)) for side_px in ICON_SIZES])
    outputs.append((ICON, None, icon))
    outputs.append((INSTALLER_ICON, None, icon))

    facts = {
        "size": size,
        "ink": ink,
        "square": (x0, y0, side),
        "frames": [
            (side_px, 40 + side_px * side_px * 4 + ((side_px + 31) // 32) * 4 * side_px)
            for side_px in ICON_SIZES
        ],
    }
    return outputs, facts


# --------------------------------------------------------------------------
# Pillow: the preferred renderer
# --------------------------------------------------------------------------


class PillowBackend:
    """Pillow does the filtering in C and the pixels stay in memory."""

    name = "Pillow"

    def __init__(self) -> None:
        import PIL
        from PIL import Image, ImageChops

        self.Image = Image
        self.ImageChops = ImageChops
        self.lanczos = getattr(Image, "Resampling", Image).LANCZOS
        self.label = f"Pillow {PIL.__version__}"

    def load(self, path: Path) -> Any:
        return self.Image.open(path).convert("RGBA")

    def decode_pixels(self, blob: bytes) -> tuple[tuple[int, int], bytes]:
        """Decode an encoded image to (size, RGBA bytes), for the check's fallback."""
        import io as _io

        with self.Image.open(_io.BytesIO(blob)) as handle:
            rgba = handle.convert("RGBA")
            return rgba.size, rgba.tobytes()

    def size(self, handle: Any) -> tuple[int, int]:
        return handle.size

    def ink_box(self, handle: Any) -> tuple[int, int, int, int]:
        above = handle.getchannel("A").point(lambda value: 255 if value > ALPHA_FLOOR else 0)
        box = above.getbbox()
        if box is None:
            raise SystemExit(f"{SOURCE.name}: no artwork above alpha {ALPHA_FLOOR}")
        return box

    def crop(self, handle: Any, box: tuple[int, int, int]) -> Any:
        x0, y0, side = box
        return handle.crop((x0, y0, x0 + side, y0 + side))

    def premultiplied(self, handle: Any) -> Any:
        alpha = handle.getchannel("A")
        planes = [
            self.ImageChops.multiply(band, alpha)
            for band in handle.convert("RGB").split()
        ]
        return planes, alpha

    def straight(self, premultiplied: Any, side: int) -> Any:
        planes, alpha = premultiplied
        scaled_alpha = alpha.resize((side, side), self.lanczos)
        scaled = [plane.resize((side, side), self.lanczos) for plane in planes]
        return compose(self.Image, scaled, scaled_alpha)

    def png(self, handle: Any) -> bytes:
        buffer = BytesIO()
        handle.save(buffer, format="PNG", optimize=True)
        return buffer.getvalue()

    def rgba(self, handle: Any, side: int) -> bytes:
        return handle.tobytes()

    def clear_pixel(self, path: Path, point: tuple[int, int]) -> Path:
        """A copy of `path` with one pixel's alpha cleared, for `--self-test`."""
        image = self.Image.open(path).convert("RGBA")
        image.putpixel(point, (0, 0, 0, 0))
        touched = path.with_name(f"{path.stem}-perturbed.png")
        image.save(touched, format="PNG")
        return touched

    def close(self) -> None:
        pass


# --------------------------------------------------------------------------
# ImageMagick: the fallback, same recipe, one process per step
# --------------------------------------------------------------------------


def find_magick() -> str | None:
    """The ImageMagick executable, if one is installed.

    Validated by its banner rather than its name: `C:\\Windows\\system32\\
    convert.exe` is the FAT-to-NTFS tool, it answers `-version` with an error,
    and reading that as ImageMagick would fail in a far less obvious place.
    `magick` is the 7.x name and `convert` the 6.x one.
    """
    for name in ("magick", "convert"):
        exe = shutil.which(name)
        if exe is None:
            continue
        try:
            done = subprocess.run(
                [exe, "-version"], capture_output=True, text=True, timeout=30
            )
        except (OSError, subprocess.SubprocessError):
            continue
        if "ImageMagick" in (done.stdout or "") + (done.stderr or ""):
            return exe
    return None


class MagickBackend:
    """The same steps as `magick` commands, each writing a scratch PNG.

    `-alpha premultiply` / `-alpha unpremultiply` are ImageMagick's own names
    for the two halves of the resampling this tool needs; the floor is one
    `-fx` pass, which is slow but only runs on the output sizes.
    """

    name = "ImageMagick"

    def __init__(self, exe: str) -> None:
        self.exe = exe
        self.label = f"ImageMagick ({exe})"
        self.scratch = tempfile.TemporaryDirectory(prefix="gen_brand_")
        self.count = 0

    def decode_pixels(self, blob: bytes) -> tuple[tuple[int, int], bytes]:
        """Decode an encoded image to (size, RGBA bytes) through ImageMagick."""
        identify = subprocess.run(
            [self.exe, "-", "-format", "%w %h", "info:"],
            input=blob,
            capture_output=True,
            check=False,
        )
        if identify.returncode != 0:
            raise ValueError("ImageMagick could not identify the blob")
        width, height = (int(part) for part in identify.stdout.split())
        raw = subprocess.run(
            [self.exe, "-", "-depth", "8", "rgba:-"],
            input=blob,
            capture_output=True,
            check=False,
        )
        if raw.returncode != 0:
            raise ValueError("ImageMagick could not decode the blob")
        return (width, height), raw.stdout

    def _run(self, arguments: list[str]) -> bytes:
        command = [self.exe, *arguments]
        done = subprocess.run(command, capture_output=True, check=False)
        if done.returncode != 0:
            detail = done.stderr.decode("utf-8", "replace").strip()
            raise SystemExit(f"{self.exe} failed: {' '.join(command)}\n{detail}")
        return done.stdout

    def _out(self) -> Path:
        self.count += 1
        return Path(self.scratch.name) / f"{self.count:02d}.png"

    def load(self, path: Path) -> Any:
        text = self._run([str(path), "-format", "%w %h", "info:"]).decode()
        width, height = (int(value) for value in text.split())
        return path, (width, height)

    def size(self, handle: Any) -> tuple[int, int]:
        return handle[1]

    def ink_box(self, handle: Any) -> tuple[int, int, int, int]:
        # A black border first, so the box is measured against black even if
        # the art reaches a corner, and then shifted back by that border.
        text = self._run(
            [
                str(handle[0]),
                "-alpha", "extract",
                "-threshold", f"{ALPHA_FLOOR}/255",
                "-bordercolor", "black",
                "-border", "1",
                "-format", "%@",
                "info:",
            ]
        ).decode().strip()
        matched = re.fullmatch(r"(\d+)x(\d+)\+(\d+)\+(\d+)", text)
        if matched is None:
            raise SystemExit(
                f"{self.exe}: cannot read the ink box from {handle[0].name}: {text!r}"
            )
        width, height, x0, y0 = (int(value) for value in matched.groups())
        if width <= 0 or height <= 0:
            raise SystemExit(f"{SOURCE.name}: no artwork above alpha {ALPHA_FLOOR}")
        left, top = x0 - 1, y0 - 1
        return left, top, left + width, top + height

    def crop(self, handle: Any, box: tuple[int, int, int]) -> Any:
        x0, y0, side = box
        out = self._out()
        self._run(
            [str(handle[0]), "-crop", f"{side}x{side}+{x0}+{y0}", "+repage", str(out)]
        )
        return out, (side, side)

    def premultiplied(self, handle: Any) -> Any:
        out = self._out()
        self._run(
            [
                str(handle[0]),
                "-alpha", "on",
                "-alpha", "premultiply",
                "-depth", "8",
                "-define", "png:color-type=6",
                str(out),
            ]
        )
        return out, handle[1]

    def straight(self, premultiplied: Any, side: int) -> Any:
        out = self._out()
        self._run(
            [
                str(premultiplied[0]),
                "-filter", "Lanczos",
                "-resize", f"{side}x{side}!",
                "-alpha", "unpremultiply",
                "-channel", "RGBA",
                "-fx", f"(a*255)<{ALPHA_FLOOR}?0:u",
                "+channel",
                "-depth", "8",
                "-define", "png:color-type=6",
                "-strip",
                str(out),
            ]
        )
        return out, (side, side)

    def png(self, handle: Any) -> bytes:
        return handle[0].read_bytes()

    def rgba(self, handle: Any, side: int) -> bytes:
        return self._run([str(handle[0]), "-depth", "8", "rgba:-"])

    def clear_pixel(self, path: Path, point: tuple[int, int]) -> Path:
        x, y = point
        touched = path.with_name(f"{path.stem}-perturbed.png")
        self._run(
            [
                str(path),
                "(", "-size", "1x1", "xc:transparent", ")",
                "-geometry", f"+{x}+{y}",
                "-compose", "Copy",
                "-composite",
                str(touched),
            ]
        )
        return touched

    def close(self) -> None:
        self.scratch.cleanup()


def choose_backend() -> Any:
    """Pillow if it imports, else ImageMagick, else a message naming both."""
    try:
        return PillowBackend()
    except ImportError:
        pass
    exe = find_magick()
    if exe is not None:
        return MagickBackend(exe)
    raise BackendMissing(
        "no imaging backend: this tool needs Pillow or ImageMagick.\n"
        "  install one of them, then re-run:\n"
        "    python -m pip install Pillow\n"
        "    winget install ImageMagick.ImageMagick   (or: scoop install imagemagick)\n"
        "  ImageMagick has to be on PATH as `magick` (7.x) or `convert` (6.x)."
    )


# --------------------------------------------------------------------------
# Reporting and the modes
# --------------------------------------------------------------------------


def digest(blob: bytes) -> str:
    return hashlib.sha256(blob).hexdigest()[:16]


def report(
    backend: Any,
    facts: dict[str, Any],
    outputs: list[tuple[Path, int | None, bytes]],
    entries: bool,
) -> None:
    """The measurements, one short line each -- the same in every mode."""
    width, height = facts["size"]
    left, top, right, bottom = facts["ink"]
    x0, y0, side = facts["square"]
    ink_w, ink_h = right - left, bottom - top
    print(f"backend   {backend.label}")
    print(f"source    {SOURCE.name} {width}x{height}")
    print(
        f"ink box   {left},{top}..{right - 1},{bottom - 1} "
        f"({ink_w}x{ink_h}, alpha > {ALPHA_FLOOR})"
    )
    print(
        f"square    {side}x{side} at {x0},{y0} "
        f"(mark {100 * max(ink_w, ink_h) / side:.1f}% of the side)"
    )
    for path, side_px, blob in outputs:
        drawn = f"{side_px}x{side_px}" if side_px else f"{len(facts['frames'])} BMP entries"
        print(f"{path.name:<13} {drawn:<17} {len(blob):>8} bytes  sha256 {digest(blob)}")
    # Each entry's computed length, always: this is the sum `brand.rs`'s icon
    # test asserts, so a change to the container shows up here as well as there.
    print(
        "entries   "
        + " ".join(f"{side_px}:{length}" for side_px, length in facts["frames"])
        + " bytes"
    )
    if entries:
        # The same numbers spelled out, because the test's arithmetic is a sum.
        for side_px, length in facts["frames"]:
            stride = ((side_px + 31) // 32) * 4
            print(
                f"  entry {side_px:>3}x{side_px:<3} {length:>7} bytes "
                f"= 40 + {side_px * side_px * 4} + {stride * side_px}"
            )


def self_test(
    backend: Any,
    baseline: list[tuple[Path, int | None, bytes]],
    facts: dict[str, Any],
) -> int:
    """Clear one source pixel in a temp copy and prove the outputs move with it.

    This is the check `--check` cannot make on its own: it shows the derived
    bytes are a function of the source's alpha rather than of anything cached,
    and it needs no write to the tree.
    """
    point = (facts["size"][0] // 2, facts["size"][1] // 2)
    with tempfile.TemporaryDirectory(prefix="gen_brand_selftest_") as scratch:
        copy = Path(scratch) / SOURCE.name
        copy.write_bytes(SOURCE.read_bytes())
        touched = backend.clear_pixel(copy, point)
        perturbed, _ = build(backend, touched)

    before = {path.name: digest(blob) for path, _, blob in baseline}
    after = {path.name: digest(blob) for path, _, blob in perturbed}
    print(f"self-test cleared alpha at {point[0]},{point[1]} in a temp copy of {SOURCE.name}")
    unchanged = [name for name in before if before[name] == after[name]]
    for name in before:
        state = "unchanged" if name in unchanged else "changed"
        print(f"self-test {name:<13} {state:<9} {before[name]} -> {after[name]}")
    if unchanged:
        print(
            f"self-test failed: {', '.join(unchanged)} ignored the source's alpha",
            file=sys.stderr,
        )
        return 1
    print(f"self-test all {len(before)} outputs follow the source's alpha")
    return 0


def same_pixels(backend, left: bytes, right: bytes) -> bool:
    """Whether two encoded images decode to the same size and the same pixels.

    `backend` is whatever `choose_backend` returned; each backend knows how to hand an image's
    pixels back as a flat sequence, which is the only comparison that is about the art rather
    than about the encoder that wrote it.
    """
    try:
        a_size, a_pixels = backend.decode_pixels(left)
        b_size, b_pixels = backend.decode_pixels(right)
    except Exception:
        # A blob the backend cannot read is not the same picture by definition.
        return False
    if a_size != b_size:
        return False
    return bytes(a_pixels) == bytes(b_pixels)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument(
        "--check", action="store_true",
        help="fail if the files on disk are not what this would write",
    )
    parser.add_argument(
        "--report", action="store_true",
        help="the measurements and each icon entry, writing nothing",
    )
    parser.add_argument(
        "--self-test", action="store_true",
        help="prove a one-pixel change to the source's alpha changes the outputs",
    )
    args = parser.parse_args()

    if not SOURCE.is_file():
        print(f"missing brand art: {SOURCE}", file=sys.stderr)
        return 2

    try:
        backend = choose_backend()
    except BackendMissing as error:
        print(str(error), file=sys.stderr)
        return 2

    try:
        outputs, facts = build(backend, SOURCE)
        if args.self_test:
            report(backend, facts, outputs, entries=False)
            return self_test(backend, outputs, facts)

        report(backend, facts, outputs, entries=args.report)

        if args.check:
            # Byte identity is the first test and the strongest one, but it is also a test of
            # the *encoder*, not of the art: two Pillow or ImageMagick versions write the same
            # pixels with different zlib settings, and a gate that fails on that is a gate that
            # cries wolf on every dependency bump. So a file whose bytes differ is then compared
            # pixel by pixel, and only a real difference -- different pixels, different size, a
            # missing file -- fails.
            stale = []
            reencoded = []
            for path, _, blob in outputs:
                if not path.is_file():
                    stale.append(path)
                    continue
                committed = path.read_bytes()
                if committed == blob:
                    continue
                if same_pixels(backend, committed, blob):
                    reencoded.append(path)
                    continue
                stale.append(path)
            for path in stale:
                print(f"{path.relative_to(ROOT)} is not what this tool writes", file=sys.stderr)
            if stale:
                print("Run `python tools/gen_brand.py` and commit the result.", file=sys.stderr)
                return 1
            for path in reencoded:
                print(
                    f"{path.relative_to(ROOT)} has the same pixels but different bytes "
                    "(the encoder differs); regenerate when convenient",
                    file=sys.stderr,
                )
            print(f"{len(outputs)} brand files are current")
            return 0

        if args.report:
            print("report only: nothing written")
            return 0

        for path, _, blob in outputs:
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(blob)
        print(f"wrote {len(outputs)} brand files")
        return 0
    finally:
        backend.close()


if __name__ == "__main__":
    sys.exit(main())
