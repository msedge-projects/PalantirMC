#!/usr/bin/env python3
"""Compose a page-state's two captures side by side, and difference them.

The companion to `tools/parity/capture.py`. That tool produces, for every page
state, `<page>-<state>-modrinth.png` and `<page>-<state>-sift.png` at the same
size. This one turns each pair into two files and a measurement:

* `<page>-<state>-compare.png` -- the two images side by side, 12 px apart on
  `#141414`, a 28 px strip above each labelled MODRINTH (left) and SIFT (right),
  and eight horizontal magenta rulers at 10 %, 20 %, ... 80 % of the capture
  height, drawn at 20 % opacity across both halves *and the gap between them*.
* `<page>-<state>-diff.png` -- the absolute per-pixel difference, amplified x4.
* a stdout summary -- size, mean absolute difference 0-255, and the percentage
  of pixels differing by more than 8. When `<dir>/report.txt` exists, it is also
  read and printed with the deltas already subtracted, because the colour and
  layout axes in `docs/PLAN-SIFT.md` section 1 are judged on exactly those
  numbers, not on an impression. capture.py writes one line per capture --
  `app=sift page=discover state=idle rect=52,88,1440,900 corners=12,14,16;...` --
  so a row is named by its page and state, the window rect and the sample colours
  are read independently of each other, and the five colour samples it records --
  the centre, then TL, TR, BL, BR -- are compared point by point and reported as
  COLOUR MATCH when every one is within +/-1 per channel. Older, free-form report
  lines are still read: `label <app> #hex` and `x=.. y=.. w=.. h=..` are picked up
  generically, and anything recognised but not attributable to an app is printed
  as unattributed rather than dropped.

Why the rulers. Two screenshots of the same page at the same size invite the eye
to accept any layout: everything "looks about right" until a heading on the right
sits 6 px higher than its twin and nothing in the picture says so. A horizontal
rule at a fixed fraction of the height turns that into a visible fact -- a feature
that should straddle a ruler line on both halves and straddles it on only one is
a typographic or padding difference, and you can read it off without a probe or a
measurement pass. That is also why the rule spans the gap: a single unbroken line
means "same height" is checked across the pair, not separately in each panel.

Why x4. A 1/255 difference is invisible on screen and, once written into a PNG, is
easy to mistake for compression noise. Multiplying by four lifts the differences
that matter (anti-aliasing edges, a 2 px offset, a one-step-wrong grey) into
something a person can see, and anything >= 64 is saturated white -- a clipped
diff pixel is a real difference, never an artefact of the amplification. The
statistics printed beside it are *not* amplified: they are the true 0-255 numbers,
because a tolerance has to be stated on the real scale.

Why the 8 threshold. Sub-pixel text rasterisation differs between a webview and
any native toolkit, so counting every pixel that differs by one would report a
large difference on a page that is correct (`docs/PLAN-SIFT.md` says as much). ">8"
is a difference big enough to be a different colour rather than a different
rasteriser; it is the number that should be near zero on a green page.

Two rendering paths, and which one ran:

* **Pillow**, when it is installed. It is the shorter path and it keeps the whole
  job in one process.
* **PowerShell + System.Drawing**, when Pillow is absent. The composition (two
  bitmaps loaded, drawn into a new one, saved as PNG) is ordinary GDI+; the diff
  statistics are computed from `LockBits`/`BitmapData` rather than `GetPixel`,
  because a `GetPixel` loop over 1440x900 is 1.3 million interop calls and takes
  minutes. Fastest first: a small C# helper compiled in-process with `Add-Type`;
  failing that, the same byte scan written in PowerShell (correct but slow, and
  it says so, and it says why). The script is written to a temp `.ps1` and run
  with `-File`: as an `-EncodedCommand` it is about 30 000 characters, near
  enough to the 32 767-character command-line limit that Windows refuses to
  start the process at all.

The fallback is not a stub kept for form's sake -- it is the only path on a
machine with a bare Python, so it is tested in its own right. Which path a given
run took is printed on the first line of stdout, and is never inferred from the
docstring; `--renderer` forces one, which is how the fallback gets exercised on a
machine that does have Pillow.

A note on detecting Pillow. The check asked for is
`importlib.util.find_spec("pillow")`, and that name is the *distribution* name,
not the import name: Pillow installs a package called `PIL`, so `find_spec("pillow")`
returns None on a machine that has Pillow installed and importable -- including the
one this was written on (PIL 11.3.0). The literal check is still tried first, and
`PIL` is tried immediately after it; without the second probe the fallback would
run on every machine, which is slower and pointless.

    python tools/parity/compose.py --help
    python tools/parity/compose.py --dir parity --page discover --state idle
    python tools/parity/compose.py --dir parity --all
    python tools/parity/compose.py --dir parity --all --renderer powershell

Exit codes: 0 all pairs composed; 1 a renderer or I/O failure; 2 a capture (or
the capture directory, or every pair under `--all`) is missing; 3 the Modrinth
and Sift captures for a pair differ in size; 4 bad arguments; 5 no usable
renderer on this machine.
"""
from __future__ import annotations

import argparse
import importlib.util
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import time
from dataclasses import dataclass, field
from pathlib import Path

# --- geometry, colour and measurement constants ----------------------------
#
# These are the numbers the tool is defined by, so they live in one place and
# are passed down to the PowerShell path rather than restated in its script.

GAP = 12                    # px of background between the two captures
STRIP = 28                  # px of label strip above each capture
BACKGROUND = (0x14, 0x14, 0x14)
RULER_RGB = (255, 0, 255)   # magenta: no UI in either app is this colour
RULER_ALPHA = 51            # 20 % of 255; blended, not painted
RULER_TENTHS = (1, 2, 3, 4, 5, 6, 7, 8)   # 10 %, ... 80 %: 8 lines
THRESHOLD = 8               # a difference bigger than this counts as "differing"
AMPLIFY = 4
LEFT_LABEL = "MODRINTH"
RIGHT_LABEL = "SIFT"
LABEL_PAD = 8

EXIT_OK = 0
EXIT_FAIL = 1
EXIT_MISSING = 2
EXIT_SIZE = 3
EXIT_USAGE = 4
EXIT_NO_RENDERER = 5

APP_LEFT = "modrinth"
APP_RIGHT = "sift"

PS_TIMEOUT = 900            # the PowerShell byte-scan tier is slow by nature
PS_OK = "SIFT-COMPOSE-OK "
PS_ERR = "SIFT-COMPOSE-ERROR "


class CaptureMissing(Exception):
    """A capture the pair needs is not on disk."""


class CaptureSizeMismatch(Exception):
    """Both captures exist but cannot be laid side by side."""


class RenderError(Exception):
    """The chosen renderer could not produce the images."""


@dataclass(frozen=True)
class Pair:
    page: str
    state: str
    modrinth: Path
    sift: Path

    @property
    def name(self) -> str:
        return f"{self.page}-{self.state}"

    def missing(self) -> list[Path]:
        return [p for p in (self.modrinth, self.sift) if not p.is_file()]


@dataclass
class Stats:
    width: int
    height: int
    mean_abs: float
    pct_over: float
    labels: str             # how the strips are labelled, or "none"
    diff_writer: str        # "pillow" / "cs" / "ps-lockbits"
    seconds: float
    note: str = ""          # anything the run should mention but not fail on


# --- which paths exist on this machine -------------------------------------

def pillow_note() -> tuple[bool, str]:
    """(usable, one-line description) for Pillow on this interpreter."""
    found = False
    for name in ("pillow", "PIL"):
        try:
            if importlib.util.find_spec(name) is not None:
                found = True
                break
        except (ImportError, ValueError):
            # A half-installed or sys.modules-hacked package raises here rather
            # than returning None; that is "not usable", not a crash.
            continue
    if not found:
        return False, "absent (neither 'pillow' nor 'PIL' resolves)"
    try:
        import PIL
    except ImportError as exc:
        return False, f"PIL present but not importable: {exc}"
    return True, f"PIL {getattr(PIL, '__version__', 'version unknown')}"


def powershell_hosts() -> list[str]:
    """Every PowerShell that could run the System.Drawing path, best first.

    Windows PowerShell 5.1 comes first because `System.Drawing` is part of the
    .NET Framework it runs on, so it always has it. `pwsh` may or may not carry
    System.Drawing.Common, so it is a later candidate -- but a candidate, because
    on a machine with no 5.1 it is the only one.
    """
    hosts: list[str] = []
    for name in ("powershell.exe", "pwsh.exe"):
        found = shutil.which(name)
        if found and found not in hosts:
            hosts.append(found)
    system_root = os.environ.get("SystemRoot", r"C:\Windows")
    classic = Path(system_root) / "System32" / "WindowsPowerShell" / "v1.0" / "powershell.exe"
    if classic.is_file() and str(classic) not in hosts:
        hosts.append(str(classic))
    return hosts


# --- the block-glyph fallback ----------------------------------------------
#
# Only needed when no font at all can be had. 5x7 cells for the ten letters the
# two labels use: enough to keep the halves identifiable, and cheap enough that
# a machine with no fonts still gets a labelled image instead of an unlabelled
# one. '1' is ink.

_GLYPHS = {
    "M": ("10001", "11011", "10101", "10101", "10001", "10001", "10001"),
    "O": ("01110", "10001", "10001", "10001", "10001", "10001", "01110"),
    "D": ("11110", "10001", "10001", "10001", "10001", "10001", "11110"),
    "R": ("11110", "10001", "10001", "11110", "10100", "10010", "10001"),
    "I": ("11111", "00100", "00100", "00100", "00100", "00100", "11111"),
    "N": ("10001", "11001", "10101", "10011", "10001", "10001", "10001"),
    "T": ("11111", "00100", "00100", "00100", "00100", "00100", "00100"),
    "H": ("10001", "10001", "10001", "11111", "10001", "10001", "10001"),
    "S": ("01111", "10000", "10000", "01110", "00001", "00001", "11110"),
    "F": ("11111", "10000", "10000", "11110", "10000", "10000", "10000"),
}


def draw_block_label(draw, text: str, x: int, right_edge: int | None = None) -> None:
    """Draw `text` from 5x7 blocks, left-aligned at `x` or right-aligned at `right_edge`."""
    scale = max(1, (STRIP - 8) // 7)
    advance = 5 * scale + scale
    width = len(text) * advance - scale
    if right_edge is not None:
        x = right_edge - width
    top = (STRIP - 7 * scale) // 2
    white = (255, 255, 255)
    for index, char in enumerate(text.upper()):
        rows = _GLYPHS.get(char)
        if rows is None:
            continue
        for row_index, row in enumerate(rows):
            for col_index, bit in enumerate(row):
                if bit != "1":
                    continue
                x0 = x + index * advance + col_index * scale
                y0 = top + row_index * scale
                draw.rectangle([x0, y0, x0 + scale - 1, y0 + scale - 1], fill=white)


# --- the Pillow path -------------------------------------------------------

def _label_font(ImageFont, draw, texts, panel_width: int):
    """The best available face for the strips, fitted to the strip height.

    The repository's own Inter-600 comes first: it is the face both apps draw
    with, so the label reads in the same type as the screenshots under it. It is
    found by glob off the crate tree rather than by a fixed path because the crate
    is being renamed (`palantir-desktop` -> `sift-desktop`) and a label strip is
    not worth breaking over a directory name.
    """
    root = Path(__file__).resolve().parents[2]
    candidates: list[Path] = sorted(root.glob("crates/*/assets/fonts/Inter-600.ttf"))
    windir = Path(os.environ.get("WINDIR", r"C:\Windows")) / "Fonts"
    for name in ("segoeuisb.ttf", "segoeuib.ttf", "arialbd.ttf", "segoeui.ttf", "arial.ttf"):
        candidates.append(windir / name)

    limit_w = panel_width - 2 * LABEL_PAD
    limit_h = STRIP - 6
    for path in candidates:
        if not path.is_file():
            continue
        for size in (13, 12, 11, 10, 9):
            try:
                font = ImageFont.truetype(str(path), size)
            except OSError:
                break           # this file is not a readable face; next candidate
            boxes = [font.getbbox(t) for t in texts]
            widest = max(draw.textlength(t, font=font) for t in texts)
            tallest = max(b[3] - b[1] for b in boxes)
            if widest <= limit_w and tallest <= limit_h:
                return font, f"text:{path.name}@{size}"

    try:
        default = ImageFont.load_default()
    except (OSError, ImportError):
        return None, ""
    if max(draw.textlength(t, font=default) for t in texts) <= limit_w:
        return default, "bitmap:default"
    return None, ""


def _draw_centred(draw, text: str, font, left_x: int, right_edge: int | None = None) -> None:
    """Draw white text vertically centred in the strip, left- or right-aligned."""
    box = font.getbbox(text)
    x = left_x - box[0]
    if right_edge is not None:
        x = right_edge - (box[2] - box[0])
    y = (STRIP - (box[3] - box[1])) // 2 - box[1]
    draw.text((x, y), text, font=font, fill=(255, 255, 255))


def render_pillow(pair: Pair, compare_out: Path, diff_out: Path) -> Stats:
    # A Pillow that resolved a moment ago but will not import now is a renderer
    # failure like any other, not a traceback out of main().
    try:
        from PIL import Image, ImageChops, ImageDraw, ImageFont
    except ImportError as exc:
        raise RenderError(f"Pillow stopped importing: {exc}") from exc

    started = time.perf_counter()
    with Image.open(pair.modrinth) as handle:
        left = handle.convert("RGB")
    with Image.open(pair.sift) as handle:
        right = handle.convert("RGB")
    if left.size != right.size:
        raise CaptureSizeMismatch(
            f"{left.size[0]}x{left.size[1]} (modrinth) vs {right.size[0]}x{right.size[1]} (sift)"
        )
    width, height = left.size

    canvas = Image.new("RGB", (width * 2 + GAP, STRIP + height), BACKGROUND)
    canvas.paste(left, (0, STRIP))
    canvas.paste(right, (width + GAP, STRIP))

    # The rulers go on last, in one RGBA overlay composited over both halves, so
    # a single compositing step produces every blended pixel (and the gap's
    # background gets the same 20 % magenta as the images).
    overlay = Image.new("RGBA", canvas.size, (0, 0, 0, 0))
    ruler = ImageDraw.Draw(overlay)
    fill = (*RULER_RGB, RULER_ALPHA)
    for tenth in RULER_TENTHS:
        y = STRIP + round(height * tenth / 10)
        ruler.line([(0, y), (canvas.width - 1, y)], fill=fill, width=1)
    canvas = Image.alpha_composite(canvas.convert("RGBA"), overlay).convert("RGB")

    strip = ImageDraw.Draw(canvas)
    font, label_note = _label_font(ImageFont, strip, (LEFT_LABEL, RIGHT_LABEL), width)
    if font is not None:
        _draw_centred(strip, LEFT_LABEL, font, LABEL_PAD)
        _draw_centred(strip, RIGHT_LABEL, font, 0, right_edge=width * 2 + GAP - LABEL_PAD)
    else:
        draw_block_label(strip, LEFT_LABEL, LABEL_PAD)
        draw_block_label(strip, RIGHT_LABEL, 0, right_edge=width * 2 + GAP - LABEL_PAD)
        label_note = "blocks"
    canvas.save(compare_out, "PNG")

    diff = ImageChops.difference(left, right)
    histogram = diff.histogram()
    pixels = width * height
    weighted = 0
    for plane in range(3):
        band = histogram[plane * 256:(plane + 1) * 256]
        weighted += sum(value * count for value, count in enumerate(band))
    mean_abs = weighted / (3.0 * pixels)

    red, green, blue = diff.split()
    peak = ImageChops.lighter(ImageChops.lighter(red, green), blue)
    mask = peak.point([255 if value > THRESHOLD else 0 for value in range(256)])
    pct_over = 100.0 * mask.histogram()[255] / pixels

    # Pillow 10+ rejects one 256-entry table on a three-band image, so the table
    # is repeated per band -- same amplification, one lookup per channel.
    amplified = [min(255, value * AMPLIFY) for value in range(256)] * 3
    diff.point(amplified).save(diff_out, "PNG")

    return Stats(
        width=width,
        height=height,
        mean_abs=mean_abs,
        pct_over=pct_over,
        labels=label_note,
        diff_writer="pillow",
        seconds=time.perf_counter() - started,
    )


# --- the PowerShell + System.Drawing path ----------------------------------

CS_SOURCE = r"""
using System;
using System.Drawing;
using System.Drawing.Imaging;
using System.Runtime.InteropServices;

// The diff, at full resolution, without one interop call per pixel: both
// bitmaps are locked as 32bppArgb (BGRA, stride w*4), copied into managed
// byte arrays once, then scanned in a tight loop. Returns
// { width, height, meanAbs, pctOver, overPixels }.
public static class SiftParityDiff
{
    public static double[] Compare(string pathA, string pathB, string outPng,
                                   int threshold, int amplify)
    {
        using (Bitmap a = new Bitmap(pathA))
        using (Bitmap b = new Bitmap(pathB))
        {
            int w = a.Width, h = a.Height;
            if (b.Width != w || b.Height != h)
                throw new ArgumentException("captures differ in size: " + w + "x" + h
                    + " vs " + b.Width + "x" + b.Height);

            Rectangle rect = new Rectangle(0, 0, w, h);
            PixelFormat fmt = PixelFormat.Format32bppArgb;
            BitmapData ad = a.LockBits(rect, ImageLockMode.ReadOnly, fmt);
            BitmapData bd = b.LockBits(rect, ImageLockMode.ReadOnly, fmt);
            byte[] pa = new byte[ad.Stride * h];
            byte[] pb = new byte[bd.Stride * h];
            Marshal.Copy(ad.Scan0, pa, 0, pa.Length);
            Marshal.Copy(bd.Scan0, pb, 0, pb.Length);
            int stride = ad.Stride;
            a.UnlockBits(ad);
            b.UnlockBits(bd);

            long sum = 0;
            long over = 0;
            using (Bitmap d = new Bitmap(w, h, fmt))
            {
                BitmapData dd = d.LockBits(rect, ImageLockMode.WriteOnly, fmt);
                byte[] pd = new byte[dd.Stride * h];
                for (int y = 0; y < h; y++)
                {
                    int s = y * stride;
                    int t = y * dd.Stride;
                    for (int x = 0; x < w; x++)
                    {
                        int i = s + x * 4;
                        int j = t + x * 4;
                        int db = pa[i] > pb[i] ? pa[i] - pb[i] : pb[i] - pa[i];
                        int dg = pa[i + 1] > pb[i + 1] ? pa[i + 1] - pb[i + 1] : pb[i + 1] - pa[i + 1];
                        int dr = pa[i + 2] > pb[i + 2] ? pa[i + 2] - pb[i + 2] : pb[i + 2] - pa[i + 2];
                        sum += db + dg + dr;
                        int mx = db;
                        if (dg > mx) mx = dg;
                        if (dr > mx) mx = dr;
                        if (mx > threshold) over++;
                        pd[j] = (byte)(db * amplify > 255 ? 255 : db * amplify);
                        pd[j + 1] = (byte)(dg * amplify > 255 ? 255 : dg * amplify);
                        pd[j + 2] = (byte)(dr * amplify > 255 ? 255 : dr * amplify);
                        pd[j + 3] = 255;
                    }
                }
                Marshal.Copy(pd, 0, dd.Scan0, pd.Length);
                d.UnlockBits(dd);
                d.Save(outPng, ImageFormat.Png);
            }
            double total = (double)w * (double)h;
            return new double[] { w, h, (double)sum / (3.0 * total),
                                  100.0 * (double)over / total, (double)over };
        }
    }
}
"""

PS_BODY = r"""
$ErrorActionPreference = 'Stop'
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8

function Send-Ok($payload) {
    Write-Output ('SIFT-COMPOSE-OK ' + ($payload | ConvertTo-Json -Compress))
}
function Send-Error([string]$message) {
    Write-Output ('SIFT-COMPOSE-ERROR ' + ((@{ message = $message }) | ConvertTo-Json -Compress))
    exit 1
}

try {
    Add-Type -AssemblyName System.Drawing -ErrorAction Stop
} catch {
    Send-Error ('System.Drawing could not be loaded: ' + $_.Exception.Message)
}

$csSource = @'
__CS_SOURCE__
'@

# The C# helper is the fast path and the one that will run on any machine with
# .NET Framework's compiler present. `-ReferencedAssemblies System.Drawing` is
# not optional: Add-Type's default reference set does not include it, and the
# compile then fails on `using System.Drawing.Imaging` -- silently, unless the
# reason is carried back to the caller, which is what $csError is for.
# If Add-Type cannot compile at all -- a locked-down box, no csc.exe -- the same
# scan runs in PowerShell below: same answer, far slower, and the writer name it
# reports says which one ran.
$csError = ''
$diffWriter = 'cs'
try {
    Add-Type -TypeDefinition $csSource -Language CSharp -ReferencedAssemblies System.Drawing -ErrorAction Stop
} catch {
    $diffWriter = 'ps-lockbits'
    $csError = $_.Exception.Message -replace "`r?`n", ' | '
}

function Invoke-LockedDiff([string]$pa, [string]$pb, [string]$outDiff, [int]$threshold, [int]$amplify) {
    $a = [System.Drawing.Bitmap]::new($pa)
    $b = [System.Drawing.Bitmap]::new($pb)
    $diff = $null
    $la = $null
    $lb = $null
    $ld = $null
    try {
        $w = $a.Width
        $h = $a.Height
        if ($b.Width -ne $w -or $b.Height -ne $h) { throw 'captures differ in size' }
        $rect = [System.Drawing.Rectangle]::new(0, 0, $w, $h)
        $fmt = [System.Drawing.Imaging.PixelFormat]::Format32bppArgb
        $la = $a.LockBits($rect, [System.Drawing.Imaging.ImageLockMode]::ReadOnly, $fmt)
        $lb = $b.LockBits($rect, [System.Drawing.Imaging.ImageLockMode]::ReadOnly, $fmt)
        $stride = $la.Stride
        $pa8 = [byte[]]::new($stride * $h)
        $pb8 = [byte[]]::new($stride * $h)
        [System.Runtime.InteropServices.Marshal]::Copy($la.Scan0, $pa8, 0, $pa8.Length)
        [System.Runtime.InteropServices.Marshal]::Copy($lb.Scan0, $pb8, 0, $pb8.Length)
        $a.UnlockBits($la)
        $la = $null
        $b.UnlockBits($lb)
        $lb = $null

        $diff = [System.Drawing.Bitmap]::new($w, $h, $fmt)
        $ld = $diff.LockBits($rect, [System.Drawing.Imaging.ImageLockMode]::WriteOnly, $fmt)
        $dstride = $ld.Stride
        $pd = [byte[]]::new($dstride * $h)
        [long]$sum = 0
        [long]$over = 0
        for ($y = 0; $y -lt $h; $y++) {
            $arow = $y * $stride
            $drow = $y * $dstride
            for ($x = 0; $x -lt $w; $x++) {
                $i = $arow + $x * 4
                $o = $drow + $x * 4
                $d0 = [math]::Abs([int]$pa8[$i] - [int]$pb8[$i])
                $d1 = [math]::Abs([int]$pa8[$i + 1] - [int]$pb8[$i + 1])
                $d2 = [math]::Abs([int]$pa8[$i + 2] - [int]$pb8[$i + 2])
                $sum += $d0 + $d1 + $d2
                $mx = $d0
                if ($d1 -gt $mx) { $mx = $d1 }
                if ($d2 -gt $mx) { $mx = $d2 }
                if ($mx -gt $threshold) { $over++ }
                $v0 = $d0 * $amplify
                if ($v0 -gt 255) { $v0 = 255 }
                $pd[$o] = [byte]$v0
                $v1 = $d1 * $amplify
                if ($v1 -gt 255) { $v1 = 255 }
                $pd[$o + 1] = [byte]$v1
                $v2 = $d2 * $amplify
                if ($v2 -gt 255) { $v2 = 255 }
                $pd[$o + 2] = [byte]$v2
                $pd[$o + 3] = 255
            }
        }
        [System.Runtime.InteropServices.Marshal]::Copy($pd, 0, $ld.Scan0, $pd.Length)
        $diff.UnlockBits($ld)
        $ld = $null
        $diff.Save($outDiff, [System.Drawing.Imaging.ImageFormat]::Png)
        $mean = [double]$sum / (3.0 * $w * $h)
        $pct = 100.0 * [double]$over / ([double]$w * [double]$h)
        return @([double]$w, [double]$h, $mean, $pct, [double]$over)
    } finally {
        if ($null -ne $la) { $a.UnlockBits($la) }
        if ($null -ne $lb) { $b.UnlockBits($lb) }
        if ($null -ne $ld) { $diff.UnlockBits($ld) }
        if ($null -ne $diff) { $diff.Dispose() }
        $a.Dispose()
        $b.Dispose()
    }
}

$imgA = $null
$imgB = $null
$canvas = $null
$g = $null
$pen = $null
$brush = $null
$font = $null
$labelNote = 'none'
$stats = $null

try {
    $imgA = [System.Drawing.Bitmap]::new($PathA)
    $imgB = [System.Drawing.Bitmap]::new($PathB)
    $w = $imgA.Width
    $h = $imgA.Height
    if ($imgB.Width -ne $w -or $imgB.Height -ne $h) {
        Send-Error ('captures differ in size: ' + $w + 'x' + $h + ' (modrinth) vs ' + $imgB.Width + 'x' + $imgB.Height + ' (sift)')
    }

    $canvasW = $w * 2 + $Gap
    $canvasH = $Strip + $h
    $canvas = [System.Drawing.Bitmap]::new($canvasW, $canvasH, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
    $g = [System.Drawing.Graphics]::FromImage($canvas)
    $g.CompositingMode = [System.Drawing.Drawing2D.CompositingMode]::SourceOver
    $g.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::NearestNeighbor
    $g.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::Half
    $g.Clear([System.Drawing.Color]::FromArgb(255, 20, 20, 20))
    $g.DrawImageUnscaled($imgA, 0, $Strip)
    $g.DrawImageUnscaled($imgB, $w + $Gap, $Strip)

    # Rulers last, so they are blended over the captures and over the gap.
    # A filled 1 px rectangle, not a 1 px pen: GDI+ centres a pen on its
    # coordinate and lands the line one row high, which would put this half of
    # the comparison a pixel out of step with the Pillow path and with itself.
    $rulerBrush = [System.Drawing.SolidBrush]::new([System.Drawing.Color]::FromArgb($RulerAlpha, 255, 0, 255))
    foreach ($tenth in ($RulerTenths -split ',')) {
        $y = $Strip + [int][math]::Round(($h * [int]$tenth) / 10.0)
        $g.FillRectangle($rulerBrush, 0, $y, $canvasW, 1)
    }
    $rulerBrush.Dispose()

    $g.TextRenderingHint = [System.Drawing.Text.TextRenderingHint]::AntiAliasGridFit
    $style = [System.Drawing.StringFormat]::GenericTypographic
    # Step the size down before giving up on text: a label that does not fit the
    # panel is a smaller label, not an unlabelled picture, and the same shrink
    # the Pillow path does keeps the two renderers' output looking alike.
    foreach ($size in @(9, 8, 7, 6)) {
        foreach ($family in @('Segoe UI', 'Arial', 'Tahoma', 'Verdana')) {
            try {
                $candidate = [System.Drawing.Font]::new($family, $size, [System.Drawing.FontStyle]::Bold, [System.Drawing.GraphicsUnit]::Point)
            } catch {
                continue
            }
            $probe = $g.MeasureString($LabelLeft, $candidate, 10000, $style)
            if ($probe.Height -le ($Strip - 6) -and $probe.Width -le ($w - 2 * $LabelPad)) {
                $font = $candidate
                break
            }
            $candidate.Dispose()
        }
        if ($null -ne $font) { break }
    }

    if ($null -ne $font) {
        $labelNote = 'font:' + $font.Name
        $brush = [System.Drawing.SolidBrush]::new([System.Drawing.Color]::White)
        $boxL = $g.MeasureString($LabelLeft, $font, 10000, $style)
        $boxR = $g.MeasureString($LabelRight, $font, 10000, $style)
        $ty = [single]((($Strip - $boxL.Height) / 2))
        $g.DrawString($LabelLeft, $font, $brush, [single]$LabelPad, $ty, $style)
        $g.DrawString($LabelRight, $font, $brush, [single]($canvasW - $LabelPad - $boxR.Width), $ty, $style)
    }

    $g.Dispose()
    $g = $null
    $canvas.Save($OutCompare, [System.Drawing.Imaging.ImageFormat]::Png)
} finally {
    if ($null -ne $g) { $g.Dispose() }
    if ($null -ne $pen) { $pen.Dispose() }
    if ($null -ne $brush) { $brush.Dispose() }
    if ($null -ne $font) { $font.Dispose() }
    if ($null -ne $canvas) { $canvas.Dispose() }
    if ($null -ne $imgA) { $imgA.Dispose() }
    if ($null -ne $imgB) { $imgB.Dispose() }
}

if ($diffWriter -eq 'cs') {
    $stats = [SiftParityDiff]::Compare($PathA, $PathB, $OutDiff, $Threshold, $Amplify)
} else {
    $stats = Invoke-LockedDiff $PathA $PathB $OutDiff $Threshold $Amplify
}

Send-Ok ([ordered]@{
    width      = [int]$stats[0]
    height     = [int]$stats[1]
    meanAbs    = [double]$stats[2]
    pctOver    = [double]$stats[3]
    overPixels = [int]$stats[4]
    labels     = $labelNote
    diffWriter = $diffWriter
    csError    = $csError
})
"""


def _ps_quote(value: str) -> str:
    """A PowerShell single-quoted literal: no interpolation, '' for a quote."""
    return "'" + str(value).replace("'", "''") + "'"


def powershell_script(pair: Pair, compare_out: Path, diff_out: Path) -> str:
    header = "\n".join(
        [
            f"$PathA = {_ps_quote(pair.modrinth)}",
            f"$PathB = {_ps_quote(pair.sift)}",
            f"$OutCompare = {_ps_quote(compare_out)}",
            f"$OutDiff = {_ps_quote(diff_out)}",
            f"$Gap = {GAP}",
            f"$Strip = {STRIP}",
            f"$RulerAlpha = {RULER_ALPHA}",
            f"$RulerTenths = {_ps_quote(','.join(str(t) for t in RULER_TENTHS))}",
            f"$Threshold = {THRESHOLD}",
            f"$Amplify = {AMPLIFY}",
            f"$LabelLeft = {_ps_quote(LEFT_LABEL)}",
            f"$LabelRight = {_ps_quote(RIGHT_LABEL)}",
            f"$LabelPad = {LABEL_PAD}",
        ]
    )
    body = PS_BODY.replace("__CS_SOURCE__", CS_SOURCE.strip("\n"))
    return header + "\n" + body


def render_powershell(pair: Pair, compare_out: Path, diff_out: Path) -> Stats:
    hosts = powershell_hosts()
    if not hosts:
        raise RenderError("no PowerShell on this machine (looked for powershell.exe, pwsh.exe)")
    script = powershell_script(pair, compare_out, diff_out)

    # A temp .ps1 rather than -EncodedCommand: the script carries the C# helper,
    # so its UTF-16/base64 form is about 30 000 characters, which is close enough
    # to the 32 767-character command-line limit that CreateProcess answers with
    # "Access is denied" instead of a useful error. A file has no such limit, and
    # it is deleted in the finally below.
    handle, script_path = tempfile.mkstemp(prefix="sift-compose-", suffix=".ps1")
    try:
        with os.fdopen(handle, "w", encoding="utf-8-sig") as stream:
            stream.write(script)
        return _run_powershell(hosts, script_path)
    finally:
        try:
            os.unlink(script_path)
        except OSError:
            # A leftover temp script is not worth failing a comparison over.
            pass


def _run_powershell(hosts: list[str], script_path: str) -> Stats:
    problems: list[str] = []
    for host in hosts:
        started = time.perf_counter()
        command = [
            host, "-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass",
            "-File", script_path,
        ]
        try:
            done = subprocess.run(
                command, capture_output=True, text=True, encoding="utf-8",
                errors="replace", timeout=PS_TIMEOUT,
            )
        except (OSError, subprocess.TimeoutExpired) as exc:
            problems.append(f"{Path(host).name}: {exc}")
            continue

        payload = None
        failure = None
        for line in done.stdout.splitlines():
            line = line.strip()
            if line.startswith(PS_OK):
                payload = json.loads(line[len(PS_OK):])
            elif line.startswith(PS_ERR):
                failure = json.loads(line[len(PS_ERR):]).get("message", line)
        if payload is not None:
            note = ""
            if payload.get("diffWriter") == "ps-lockbits" and payload.get("csError"):
                note = (
                    "the C# scanner did not compile, so the slow PowerShell scan ran: "
                    + str(payload["csError"])[:200]
                )
            return Stats(
                width=int(payload["width"]),
                height=int(payload["height"]),
                mean_abs=float(payload["meanAbs"]),
                pct_over=float(payload["pctOver"]),
                labels=str(payload["labels"]),
                diff_writer=str(payload["diffWriter"]),
                seconds=time.perf_counter() - started,
                note=note,
            )
        detail = failure or (done.stderr.strip().splitlines() or ["no output"])[-1]
        problems.append(f"{Path(host).name} (exit {done.returncode}): {detail}")

    raise RenderError("; ".join(problems))


# --- choosing a renderer ---------------------------------------------------

def choose_renderer(requested: str) -> tuple[str, str, object]:
    """(name, one-line detail, callable) for the renderer this run will use."""
    usable, note = pillow_note()
    if requested in ("auto", "pillow") and usable:
        return "pillow", note, render_pillow
    if requested == "pillow":
        raise RenderError(f"--renderer pillow was asked for, but Pillow is {note}")
    hosts = powershell_hosts()
    if requested in ("auto", "powershell") and hosts:
        return "powershell", f"{Path(hosts[0]).name}, System.Drawing", render_powershell
    if requested == "powershell":
        raise RenderError("--renderer powershell was asked for, but no PowerShell was found")
    raise RenderError(f"no renderer available: Pillow is {note} and no PowerShell was found")


# --- finding the pairs -----------------------------------------------------

def known_pages(out_dir: Path) -> list[str]:
    """Page names from `pages.json`, longest first, or [] if it cannot be read.

    `--all` has to split `<page>-<state>` out of a filename, and page names and
    state names both contain hyphens (`instance-details`, `hover`, `tab-modrinth`),
    so a filename alone is ambiguous. pages.json is the registry capture.py drives
    itself from, which makes it the only authority on where a page name ends. It
    is read leniently -- list of strings, list of objects, or an object -- because
    the layout is capture.py's to choose and this tool must not break when it
    changes.
    """
    for candidate in (out_dir / "pages.json", Path(__file__).with_name("pages.json")):
        if not candidate.is_file():
            continue
        try:
            data = json.loads(candidate.read_text(encoding="utf-8"))
        except (OSError, json.JSONDecodeError) as exc:
            print(f"pages.json  ignored: {exc}", file=sys.stderr)
            continue
        # pages.json is {"_comment": [...], "constants": {...}, "pages": {...}}.
        # Only the names under "pages" are page names: the constant table and the
        # comment lines would otherwise be taken as page names too, and a page
        # name is the only thing this list is used for.
        source = data.get("pages", data) if isinstance(data, dict) else data
        if isinstance(source, dict):
            # The page names are exactly this dict's keys; its values hold step
            # lists whose words are not page names.
            names = [key for key in source if not key.startswith("_")]
        else:
            names = []
            _collect_names(source, names)
        if names:
            return sorted(set(names), key=len, reverse=True)
    return []


def _collect_names(node, out: list[str]) -> None:
    """Every plausible page name under `node`: dict keys and short strings."""
    if isinstance(node, str):
        if 0 < len(node) <= 40 and " " not in node:
            out.append(node)
    elif isinstance(node, list):
        for item in node:
            _collect_names(item, out)
    elif isinstance(node, dict):
        for key, value in node.items():
            if not key.startswith("_"):
                out.append(key)
            _collect_names(value, out)


def split_name(stem: str, pages: list[str]) -> tuple[str, str]:
    """(page, state) out of a capture's stem, or ("", "") if it is unusable."""
    for page in pages:
        prefix = page + "-"
        if stem.startswith(prefix) and len(stem) > len(prefix):
            return page, stem[len(prefix):]
    if "-" not in stem:
        return "", ""
    page, state = stem.rsplit("-", 1)
    return (page, state) if page and state else ("", "")


def discover(out_dir: Path, pages: list[str]) -> tuple[list[Pair], list[str], list[str]]:
    """(pairs, gaps, notes) for every capture under `out_dir`.

    `gaps` are captures with no partner: capture.py was asked for that pair and
    did not produce both halves, which is a missing capture and exits 2 rather
    than being a footnote. `notes` are things recovered from or skipped -- a
    filename whose page and state cannot be read, or two files claiming one
    state -- which are worth saying but are not a failure of the capture run.
    """
    slots: dict[tuple[str, str], dict[str, Path]] = {}
    gaps: list[str] = []
    notes: list[str] = []
    for app in (APP_LEFT, APP_RIGHT):
        for path in sorted(out_dir.glob(f"*-{app}.png")):
            stem = path.name[: -len(f"-{app}.png")]
            page, state = split_name(stem, pages)
            if not page:
                notes.append(f"cannot read a page and state out of {path.name}; skipped")
                continue
            slot = slots.setdefault((page, state), {})
            if app in slot:
                notes.append(f"{path.name} and {slot[app].name} both claim {page}/{state}; kept the first")
                continue
            slot[app] = path

    pairs: list[Pair] = []
    for (page, state), slot in slots.items():
        left = slot.get(APP_LEFT)
        right = slot.get(APP_RIGHT)
        if left is None or right is None:
            have = left or right
            wanted = f"{page}-{state}-{'sift' if left else 'modrinth'}.png"
            gaps.append(f"{have.name}: missing {wanted}")
            continue
        pairs.append(Pair(page, state, left, right))

    order = {name: index for index, name in enumerate(reversed(pages))}
    pairs.sort(key=lambda p: (order.get(p.page, 0), p.page, p.state))
    return pairs, gaps, notes


# --- report.txt: sampled colours and window rects --------------------------

COLOUR_HEX = re.compile(r"#([0-9a-fA-F]{6})(?:[0-9a-fA-F]{2})?\b")
COLOUR_TRIPLE = re.compile(r"(?<![\d.])(\d{1,3})\s*,\s*(\d{1,3})\s*,\s*(\d{1,3})(?![\d.])")
# The whole colour token, so a label can be cut where the colour begins rather
# than at its first digit -- `accent rgb(27, 217, 106)` must yield "accent", not
# "accent rgb(".
COLOUR_TOKEN = re.compile(
    r"#[0-9a-fA-F]{6}\b"
    r"|rgba?\s*\([^)]*\)"
    r"|(?<![\d.])\d{1,3}\s*,\s*\d{1,3}\s*,\s*\d{1,3}\b"
)
APP_TOKEN = re.compile(rb"\b(modrinth|sift)\b".decode())
RECT_KEYS = {
    "x": ("left", "x", "l"),
    "y": ("top", "y", "t"),
    "w": ("width", "w"),
    "h": ("height", "h"),
}
# capture.py's own shape: one `key=value` line per capture,
#   app=sift page=discover state=idle rect=52,88,1440,900 corners=12,14,16;13,15,17;...
# The two values with commas in them are matched separately from the rest, so a
# rect and a corners list on the same line are read independently.
CAPTURE_KEYS = re.compile(r"(\w+)=([^\s=]+)")
CAPTURE_RECT = re.compile(r"\brect=(-?\d+),(-?\d+),(-?\d+),(-?\d+)")
CAPTURE_CORNERS = re.compile(r"\bcorners=([0-9,;\s]+)")
# capture.py's `corners` is five samples: the centre, then the four inset corners
# in reading order. It validates the count itself (`len(corners) != 5` raises), so
# five is the shape this reads; four is still labelled as the four insets in case
# a capture from before that change is compared.
CORNER_NAMES_FIVE = ("C", "TL", "TR", "BL", "BR")
CORNER_NAMES_FOUR = ("TL", "TR", "BL", "BR")
COLOUR_TOLERANCE = 1                         # +/-1 per channel is the colour axis' tolerance


@dataclass
class Reading:
    """One measured thing, per app: samples pair up on their label's skeleton."""

    label: str = ""
    colour: tuple[int, int, int] | None = None
    rect: tuple[int, int, int, int] | None = None
    seen: int = 0


@dataclass
class Capture:
    """One `app=... page=... state=...` line: a window rect and four corner colours."""

    page: str
    state: str
    app: str
    rect: tuple[int, int, int, int] | None = None
    corners: list[tuple[int, int, int]] = field(default_factory=list)
    line: str = ""



@dataclass
class Report:
    lines: int = 0
    captures: dict[tuple[str, str], dict[str, Capture]] = field(default_factory=dict)
    colours: dict[str, dict[str, Reading]] = field(default_factory=dict)
    rects: dict[str, dict[str, Reading]] = field(default_factory=dict)
    loose: list[str] = field(default_factory=list)
    sample_lines: list[str] = field(default_factory=list)

    @property
    def found(self) -> bool:
        return bool(self.captures or self.colours or self.rects or self.loose)


def _key(label: str) -> str:
    """A label's alphanumeric skeleton: 'rail.bg' and 'rail bg' are the same key.

    The two apps punctuate their own reports differently, and a near-miss in
    punctuation should not hide a colour delta that is actually being measured.
    """
    return re.sub(r"[^0-9a-z]+", " ", label.casefold()).strip()


def _label_of(text: str) -> str:
    """The name of the thing measured: the text before the first measurement.

    A capture script writes its measurements either `label app #hex` or
    `label #hex app`, so the label is whatever leads the line -- but only up to
    the first colour or rectangle, not the whole chunk.
    """
    starts = [match.start() for match in COLOUR_TOKEN.finditer(text)]
    rect = re.search(r"\b(?:left|top|width|height|rect)\b", text, re.IGNORECASE)
    if rect is not None:
        starts.append(rect.start())
    cut = min(starts) if starts else len(text)
    return text[:cut].strip().rstrip(" .:;,-|=")


def _split_by_app(line: str) -> tuple[str, list[tuple[str, str]]]:
    """(label prefix, [(app, chunk)]) for one line of report.txt.

    Splitting at each app name handles both layouts a capture script would
    plausibly write: one app per line, and both apps on one line with their
    difference beside them. A measurement written *before* the app name is
    attributed to the first app the line names.
    """
    tokens = list(APP_TOKEN.finditer(line))
    if not tokens:
        return "", [("", line)]
    prefix = line[: tokens[0].start()].strip()
    chunks: list[tuple[str, str]] = []
    if prefix:
        chunks.append((tokens[0].group(1), prefix))
    for index, token in enumerate(tokens):
        end = tokens[index + 1].start() if index + 1 < len(tokens) else len(line)
        chunks.append((token.group(1), line[token.end():end]))
    return prefix, chunks


def _parse_colours(chunk: str) -> list[tuple[int, int, int]]:
    found: list[tuple[int, int, int]] = []
    for match in COLOUR_HEX.finditer(chunk):
        digits = match.group(1)
        found.append((int(digits[0:2], 16), int(digits[2:4], 16), int(digits[4:6], 16)))
    if found:
        return found
    for match in COLOUR_TRIPLE.finditer(chunk):
        rgb = tuple(int(match.group(index)) for index in (1, 2, 3))
        if max(rgb) <= 255:
            found.append(rgb)  # type: ignore[arg-type]
    return found


def _parse_capture(line: str) -> Capture | None:
    """The `app=... page=... state=...` line capture.py writes, or None.

    Everything is read from the keys rather than from position: the page and
    state are what a row should be *called*, and the app token is a value here,
    not the separator between two columns, so the generic label/colour reader
    would collapse every one of these lines into a row named `app=`.
    """
    fields = dict(CAPTURE_KEYS.findall(line))
    app = fields.get("app", "").casefold()
    page = fields.get("page")
    if app not in (APP_LEFT, APP_RIGHT) or not page:
        return None

    corners: list[tuple[int, int, int]] = []
    match = CAPTURE_CORNERS.search(line)
    if match is not None:
        for chunk in match.group(1).split(";"):
            parts = [part.strip() for part in chunk.split(",")]
            if len(parts) == 3 and all(part.isdigit() for part in parts):
                rgb = (int(parts[0]), int(parts[1]), int(parts[2]))
                if max(rgb) <= 255:
                    corners.append(rgb)

    rect = None
    match = CAPTURE_RECT.search(line)
    if match is not None:
        rect = (int(match.group(1)), int(match.group(2)), int(match.group(3)), int(match.group(4)))

    return Capture(
        page=page,
        state=fields.get("state", ""),
        app=app,
        rect=rect,
        corners=corners,
        line=line,
    )


def _parse_rect(chunk: str) -> tuple[int, int, int, int] | None:
    """(x, y, w, h) from `x=.. y=.. w=.. h=..` or, for a line that says "rect",
    from the first four integers on it. Two shapes, because those are the two a
    capture script writes without thinking about it."""
    named: list[int] = []
    for aliases in RECT_KEYS.values():
        pattern = r"\b(?:" + "|".join(aliases) + r")\s*[=:]\s*(-?\d+)"
        match = re.search(pattern, chunk, re.IGNORECASE)
        if match is None:
            named = []
            break
        named.append(int(match.group(1)))
    if len(named) == 4:
        return (named[0], named[1], named[2], named[3])
    if "rect" in chunk.casefold():
        numbers = re.findall(r"-?\d+", chunk)
        if len(numbers) >= 4:
            return (int(numbers[0]), int(numbers[1]), int(numbers[2]), int(numbers[3]))
    return None


def parse_report(text: str) -> Report:
    report = Report(lines=len(text.splitlines()))
    for raw in text.splitlines():
        line = raw.strip()
        if not line or set(line) <= set("-=#* ") and not COLOUR_HEX.search(line):
            continue
        if len(report.sample_lines) < 6:
            report.sample_lines.append(line)
        capture = _parse_capture(line)
        if capture is not None:
            report.captures.setdefault((capture.page, capture.state), {})[capture.app] = capture
            continue
        prefix, chunks = _split_by_app(line)
        for app, chunk in chunks:
            chunk = chunk.strip()
            if not chunk:
                continue
            label = _label_of(prefix) or _label_of(chunk)
            key = _key(label) or "?"
            box = _parse_rect(chunk)
            # A measurement with no app named on its line cannot be paired, but
            # dropping it would hide something capture.py bothered to measure.
            if not app:
                if box is not None:
                    report.loose.append(f"{label or '?'}  rect x,y,w,h = {','.join(str(v) for v in box)}")
                else:
                    for rgb in _parse_colours(chunk):
                        report.loose.append(f"{label or '?'}  colour {_hex(rgb)}")
                continue
            if box is not None:
                record = report.rects.setdefault(key, {}).setdefault(app, Reading(label))
                if record.seen == 0:
                    record.rect = box
                    record.seen = 1
                continue
            for rgb in _parse_colours(chunk):
                record = report.colours.setdefault(key, {}).setdefault(app, Reading(label))
                if record.seen == 0:
                    record.colour = rgb
                    record.seen = 1
    return report


def _hex(rgb: tuple[int, int, int]) -> str:
    return "#{:02x}{:02x}{:02x}".format(*rgb)


def _print_colour_table(report: Report) -> None:
    if not report.colours:
        return
    width = min(max([len(item.label) for row in report.colours.values()
                     for item in row.values()] + [16]), 40)
    print("  " + f"{'sampled colours':<{width}}  {'modrinth':<10}{'sift':<10}{'delta (r,g,b)':<15}{'max':>4}")
    for row in report.colours.values():
        label = next(iter(row.values())).label[:width]
        left = row.get(APP_LEFT)
        right = row.get(APP_RIGHT)
        left_text = _hex(left.colour) if left and left.colour else "-"
        right_text = _hex(right.colour) if right and right.colour else "-"
        delta = ""
        peak = ""
        if left and right and left.colour and right.colour:
            step = tuple(r - l for l, r in zip(left.colour, right.colour))
            delta = f"({step[0]:+d},{step[1]:+d},{step[2]:+d})"
            peak = str(max(abs(value) for value in step))
        print("  " + f"{label:<{width}}  {left_text:<10}{right_text:<10}{delta:<15}{peak:>4}")


def _print_rect_table(report: Report) -> None:
    if not report.rects:
        return
    width = min(max([len(item.label) for row in report.rects.values()
                     for item in row.values()] + [16]), 40)
    print("  " + f"{'window rects':<{width}}  {'modrinth':<18}{'sift':<18}delta x,y,w,h")
    for row in report.rects.values():
        label = next(iter(row.values())).label[:width]
        left = row.get(APP_LEFT)
        right = row.get(APP_RIGHT)
        left_text = ",".join(str(v) for v in left.rect) if left and left.rect else "-"
        right_text = ",".join(str(v) for v in right.rect) if right and right.rect else "-"
        delta = ""
        if left and right and left.rect and right.rect:
            step = tuple(r - l for l, r in zip(left.rect, right.rect))
            delta = ",".join(f"{value:+d}" for value in step)
        print("  " + f"{label:<{width}}  {left_text:<18}{right_text:<18}{delta}")


def _rgb_text(rgb: tuple[int, int, int]) -> str:
    return ",".join(str(value) for value in rgb)


def _corner_names(count: int) -> tuple[str, ...]:
    """Names for a corners list of this length, or numbered slots if unfamiliar."""
    if count == 5:
        return CORNER_NAMES_FIVE
    if count == 4:
        return CORNER_NAMES_FOUR
    return tuple(f"#{index + 1}" for index in range(count))


def _delta_text(step: tuple[int, int, int]) -> str:
    """Signed, so a delta reads as a direction rather than as another sample."""
    return f"{step[0]:+d},{step[1]:+d},{step[2]:+d}"


def _rect_text(box: tuple[int, int, int, int] | None) -> str:
    return ",".join(str(value) for value in box) if box else "-"


def _peak(step: tuple[int, int, int]) -> int:
    return max(abs(value) for value in step)


def _print_capture_table(report: Report) -> None:
    """One block per page and state: the window rect, then the sampled corners.

    The corners are the colour axis of the parity plan (`docs/PLAN-SIFT.md`
    section 1) in the form capture.py records it: the centre followed by the four
    inset corners in reading order, TL, TR, BL, BR. The delta beside each one is
    what that axis is judged on, and the verdict below them counts the points it
    actually compared.
    """
    if not report.captures:
        return
    for (page, state), row in report.captures.items():
        print(f"  {(page + ' ' + state).strip()}")
        left = row.get(APP_LEFT)
        right = row.get(APP_RIGHT)
        left_rect = left.rect if left else None
        right_rect = right.rect if right else None
        delta = ""
        if left_rect and right_rect:
            step = tuple(b - a for a, b in zip(left_rect, right_rect))
            delta = f"delta x{step[0]:+d} y{step[1]:+d} w{step[2]:+d} h{step[3]:+d}"
        print("    " + f"{'rect':<9}modrinth {_rect_text(left_rect):<20}sift {_rect_text(right_rect):<20}{delta}")

        left_corners = left.corners if left else []
        right_corners = right.corners if right else []
        names = _corner_names(max(len(left_corners), len(right_corners)))
        for index, name in enumerate(names):
            left_text = _rgb_text(left_corners[index]) if index < len(left_corners) else "-"
            right_text = _rgb_text(right_corners[index]) if index < len(right_corners) else "-"
            delta = ""
            if index < len(left_corners) and index < len(right_corners):
                step = tuple(b - a for a, b in zip(left_corners[index], right_corners[index]))
                flag = "ok" if _peak(step) <= COLOUR_TOLERANCE else "off"
                delta = f"{_delta_text(step)}  ({flag} at +/-{COLOUR_TOLERANCE})"
            print("    " + f"{name:<9}modrinth {left_text:<20}sift {right_text:<20}{delta}")

        if left_corners and right_corners:
            compared = min(len(left_corners), len(right_corners))
            steps = [tuple(b - a for a, b in zip(one, two))
                     for one, two in zip(left_corners, right_corners)]
            off = sum(1 for step in steps if _peak(step) > COLOUR_TOLERANCE)
            worst = max(_peak(step) for step in steps)
            points = f"all {compared} points within +/-{COLOUR_TOLERANCE}/channel"
            if off == 0:
                print(f"    colour    COLOUR MATCH  {points}")
            else:
                print(f"    colour    {off} of {compared} points more than "
                      f"+/-{COLOUR_TOLERANCE} off, worst {worst}/255")
        else:
            # One side has no corner samples -- say which, so the gap is not
            # mistaken for a match.
            missing = "modrinth" if not left_corners else "sift"
            print(f"    colour    only one app reported corner samples for this page and state "
                  f"(no {missing} corners)")


def print_report(out_dir: Path) -> None:
    path = out_dir / "report.txt"
    if not path.is_file():
        return
    try:
        text = path.read_text(encoding="utf-8", errors="replace")
    except OSError as exc:
        print(f"report.txt  unreadable ({exc})", file=sys.stderr)
        return
    report = parse_report(text)
    colours = sum(len(row) for row in report.colours.values())
    rects = sum(len(row) for row in report.rects.values())
    parts = []
    if report.captures:
        parts.append(f"{len(report.captures)} page/state pairs")
    if colours:
        parts.append(f"{colours} sampled colours")
    if rects:
        parts.append(f"{rects} rects")
    if report.loose:
        parts.append(f"{len(report.loose)} unattributed")
    print()
    print(f"report.txt  {report.lines} lines  ->  " + (", ".join(parts) or "nothing recognised"))
    if not report.found:
        # Never guess at another tool's format silently: show enough of the file
        # that whoever owns capture.py can see what this parser was handed.
        print("  no sampled colours, window rects or capture rows were recognised; first lines as written:")
        for line in report.sample_lines:
            print(f"    {line[:110]}")
        return
    _print_capture_table(report)
    _print_colour_table(report)
    _print_rect_table(report)
    if report.loose:
        print("  unattributed (the line names no app):")
        for item in report.loose:
            print(f"    {item}")


# --- output ----------------------------------------------------------------

def print_pair(pair: Pair, stats: Stats, compare_out: Path, diff_out: Path) -> None:
    print()
    print(f"{pair.page} / {pair.state}{'':<2}[{stats.seconds:.2f}s]")
    print(f"  captures  {stats.width}x{stats.height} + {stats.width}x{stats.height}")
    print(f"  compare   {compare_out.name:<34}{stats.width * 2 + GAP}x{STRIP + stats.height}")
    print(f"  diff      {diff_out.name:<34}{stats.width}x{stats.height}  writer {stats.diff_writer}")
    print(f"  labels    {stats.labels}")
    print(f"  mean-abs  {stats.mean_abs:.3f} / 255      pixels>{THRESHOLD}  {stats.pct_over:.2f} %")
    if stats.note:
        print(f"  note      {stats.note}")


def print_totals(results: list[tuple[Pair, Stats]]) -> None:
    if len(results) < 2:
        return
    means = sorted(stats.mean_abs for _, stats in results)
    middle = means[len(means) // 2] if len(means) % 2 else (means[len(means) // 2 - 1] + means[len(means) // 2]) / 2
    worst = max(results, key=lambda item: item[1].pct_over)
    print()
    print(f"{'pair':<28}{'mean-abs':>10}{'pixels>8':>10}")
    for pair, stats in results:
        print(f"{pair.page + '/' + pair.state:<28}{stats.mean_abs:>10.3f}{stats.pct_over:>10.2f}%")
    print(
        f"summary  {len(results)} pairs   mean-abs min {means[0]:.3f}  median {middle:.3f}  "
        f"max {means[-1]:.3f}   worst pixels>{THRESHOLD}: {worst[0].page}/{worst[0].state} "
        f"{worst[1].pct_over:.2f} %"
    )


def report_missing(missing: list[Path], out_dir: Path) -> None:
    print(f"capture missing under {out_dir}:", file=sys.stderr)
    for path in missing:
        print(f"  {path.name}", file=sys.stderr)
    print("run tools/parity/capture.py for that page and state first", file=sys.stderr)


# --- command line ----------------------------------------------------------

def build_parser() -> argparse.ArgumentParser:
    usable, pillow = pillow_note()
    hosts = powershell_hosts()
    path_now = (
        f"pillow ({pillow})" if usable
        else f"powershell ({Path(hosts[0]).name})" if hosts
        else "nothing -- neither Pillow nor PowerShell is installed"
    )
    parser = _Parser(
        prog="compose.py",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        description=(
            "Compose the Modrinth and Sift captures of each page state side by side and\n"
            "difference them. Writes <page>-<state>-compare.png and <page>-<state>-diff.png\n"
            "next to the captures, and prints the measurements."
        ),
        epilog=(
            "examples:\n"
            "  python tools/parity/compose.py --dir parity --page discover --state idle\n"
            "  python tools/parity/compose.py --dir parity --all\n"
            "  python tools/parity/compose.py --dir parity --all --renderer powershell\n"
            "\n"
            "render path on this interpreter: " + path_now + "\n"
            "\n"
            "exit codes: 0 ok, 1 renderer or I/O failure, 2 capture missing,\n"
            "            3 captures differ in size, 4 bad arguments, 5 no renderer\n"
        ),
    )
    parser.add_argument(
        "--dir", metavar="OUT", type=Path, default=Path("parity"),
        help="directory capture.py wrote into (default: parity)",
    )
    parser.add_argument("--page", metavar="NAME", help="page name, as capture.py writes it")
    parser.add_argument("--state", metavar="NAME", help="state name, as capture.py writes it")
    parser.add_argument(
        "--all", action="store_true",
        help="every matched pair in --dir (page and state read from the filenames)",
    )
    parser.add_argument(
        "--renderer", choices=("auto", "pillow", "powershell"), default="auto",
        help=(
            "force a rendering path; 'auto' prefers Pillow and falls back to "
            "PowerShell + System.Drawing (forcing 'powershell' is how that path "
            "gets tested on a machine that has Pillow)"
        ),
    )
    return parser


class _Parser(argparse.ArgumentParser):
    """argparse exits 2 on a usage error; this tool reserves 2 for a missing capture."""

    def error(self, message: str):
        self.print_usage(sys.stderr)
        print(f"{self.prog}: error: {message}", file=sys.stderr)
        raise SystemExit(EXIT_USAGE)


def main(argv: list[str]) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)

    if args.all and (args.page or args.state):
        parser.error("--all cannot be combined with --page or --state")
    if not args.all and not (args.page and args.state):
        parser.error("give both --page and --state, or --all")
    if (args.page and not args.state) or (args.state and not args.page):
        parser.error("--page and --state go together")

    out_dir = args.dir.expanduser()
    if not out_dir.is_dir():
        print(f"no capture directory at {out_dir}", file=sys.stderr)
        return EXIT_MISSING

    try:
        name, detail, render = choose_renderer(args.renderer)
    except RenderError as exc:
        print(f"renderer: {exc}", file=sys.stderr)
        return EXIT_NO_RENDERER

    print(f"compose  {out_dir.resolve()}   renderer: {name} ({detail})")
    print(f"         gap {GAP} px   strip {STRIP} px   ruler {len(RULER_TENTHS)} lines at "
          f"{RULER_ALPHA / 255:.0%} magenta   diff x{AMPLIFY}, threshold {THRESHOLD}")

    notes: list[str] = []
    gaps: list[str] = []
    if args.all:
        pairs, gaps, notes = discover(out_dir, known_pages(out_dir))
        if not pairs:
            for gap in gaps:
                print(f"  {gap}", file=sys.stderr)
            for note in notes:
                print(f"  {note}", file=sys.stderr)
            print(f"no complete capture pairs in {out_dir}", file=sys.stderr)
            return EXIT_MISSING
    else:
        pairs = [
            Pair(
                args.page, args.state,
                out_dir / f"{args.page}-{args.state}-{APP_LEFT}.png",
                out_dir / f"{args.page}-{args.state}-{APP_RIGHT}.png",
            )
        ]
        missing = pairs[0].missing()
        if missing:
            report_missing(missing, out_dir)
            return EXIT_MISSING

    results: list[tuple[Pair, Stats]] = []
    incomplete = bool(gaps)
    for gap in gaps:
        print(f"capture missing: {gap}", file=sys.stderr)
    for pair in pairs:
        missing = pair.missing()
        if missing:
            # --all reports one incomplete pair and carries on with the rest.
            report_missing(missing, out_dir)
            incomplete = True
            continue
        compare_out = out_dir / f"{pair.name}-compare.png"
        diff_out = out_dir / f"{pair.name}-diff.png"
        try:
            stats = render(pair, compare_out, diff_out)
        except CaptureSizeMismatch as exc:
            print(f"{pair.name}: captures differ in size: {exc}", file=sys.stderr)
            return EXIT_SIZE
        except RenderError as exc:
            print(f"{pair.name}: {exc}", file=sys.stderr)
            return EXIT_FAIL
        except OSError as exc:
            print(f"{pair.name}: {exc}", file=sys.stderr)
            return EXIT_FAIL
        print_pair(pair, stats, compare_out, diff_out)
        if stats.labels == "none":
            sidecar = out_dir / f"{pair.name}-labels.txt"
            sidecar.write_text(
                f"left  (0..{stats.width - 1})            {LEFT_LABEL}\n"
                f"right ({stats.width + GAP}..{2 * stats.width + GAP - 1})  {RIGHT_LABEL}\n",
                encoding="utf-8",
            )
            print(f"  labels    no font could be drawn; halves are named in {sidecar.name}")
        results.append((pair, stats))

    print_totals(results)
    print_report(out_dir)
    for note in notes:
        print(f"note: {note}", file=sys.stderr)

    if incomplete or not results:
        return EXIT_MISSING
    print()
    print(f"wrote {len(results)} compare + {len(results)} diff image(s) to {out_dir.resolve()}")
    return EXIT_OK


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
