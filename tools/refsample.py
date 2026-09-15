"""Measure a window capture: where its surfaces are, and what they are made of.

The companion to `tools/refocr.ps1`. OCR says *what* the reference client's
labels are; this says where its surfaces are, how tall each control is, what
colours the layers are, what corner radii they carry and what size the type is --
all of it off the pixels, none of it from the reference's source.

Why a library and not a script of coordinates: `panel_gate.py` already learned
this lesson twice over. Fixed offsets that sat inside one panel's padding landed
on a card border at another DPI scale, so every boundary this module reports is
*found* -- columns and rows are segmented where their mean colour actually
changes -- and the numbers are then read out of those segments. The structural
helpers are imported from `panel_gate` rather than reimplemented so a new
measurement means the same thing as the ones already asserted there.

    python tools/refsample.py .scratch/ref-10-home.png
    python tools/refsample.py .scratch/ref-10-home.png --json out.json
    python tools/refsample.py .scratch/ref-10-home.png --blocks 0 76 0 700

Every function takes and returns pixels and numbers. Nothing here clicks,
focuses, scrolls or writes to any window.
"""
from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

import numpy as np
from PIL import Image

sys.path.insert(0, str(Path(__file__).resolve().parent))
import panel_gate as PG  # noqa: E402  (structural helpers live with the oracle)

HERE = Path(__file__).resolve().parent

# A colour is "the same" as another when every channel is within this. WebView2
# renders a flat fill flat; the slack is for the subpixel edge of a border.
TOL = 12

# How much of a row has to differ from its band's background before the row is
# content rather than gutter. A 1px border is 1/76 of a narrow band, so this
# cannot be so high that a rule disappears, nor so low that antialiasing counts.
ROW_FRACTION = 0.06


def load(path: str | Path) -> Image.Image:
    """A capture as RGB, with any native window frame trimmed off.

    The trim is `panel_gate`'s, deliberately: the reference's captures come back
    with a DWM frame and a black border, and a stray black edge would otherwise
    become "the sidebar's background" and move every sample by a few pixels.
    """
    image = Image.open(path).convert("RGB")
    trimmed, inset = PG.crop_window_frame(image)
    return trimmed


def array(image: Image.Image) -> np.ndarray:
    return np.asarray(image, dtype=np.int16)


def hexof(rgb) -> str:
    return "#%02x%02x%02x" % (int(rgb[0]), int(rgb[1]), int(rgb[2]))


def close(a, b, tol: int = TOL) -> bool:
    return bool(np.max(np.abs(np.asarray(a, dtype=np.int16) - np.asarray(b, dtype=np.int16))) <= tol)


def dominant(a: np.ndarray, box=None, n: int = 10) -> list[dict]:
    """The most common colours in a region, most common first, with coverage.

    Reads a box as (x0, y0, x1, y1); the whole image when it is None. Coverage is
    the fraction of the region, which is what says whether a colour is a surface
    or an antialiased edge: a surface is tens of percent, an edge is a rounding
    error and never appears here at all.
    """
    if box is None:
        region = a
    else:
        x0, y0, x1, y1 = box
        region = a[max(0, y0):y1, max(0, x0):x1]
    flat = region.reshape(-1, 3)
    if flat.size == 0:
        return []
    packed = (flat[:, 0].astype(np.int32) << 16) | (flat[:, 1].astype(np.int32) << 8) | flat[:, 2]
    values, counts = np.unique(packed, return_counts=True)
    order = np.argsort(counts)[::-1][:n]
    total = flat.shape[0]
    out = []
    for i in order:
        v = int(values[i])
        out.append({
            "hex": hexof(((v >> 16) & 255, (v >> 8) & 255, v & 255)),
            "rgb": [(v >> 16) & 255, (v >> 8) & 255, v & 255],
            "count": int(counts[i]),
            "coverage": round(float(counts[i]) / total, 4),
        })
    return out


def bg(a: np.ndarray, box=None) -> tuple[int, int, int]:
    """The modal colour of a region: its background, by definition of mode."""
    top = dominant(a, box, n=1)
    if not top:
        return (0, 0, 0)
    return tuple(top[0]["rgb"])


def _segments_from_edges(length: int, edges: list[int]) -> list[tuple[int, int]]:
    bounds = [0] + [e for e in edges if 0 < e < length] + [length]
    return [(bounds[i], bounds[i + 1]) for i in range(len(bounds) - 1)]


def bands(a: np.ndarray, axis: str = "x", threshold: float = 9.0, min_len: int = 3) -> list[dict]:
    """Split the image into runs of columns (or rows) of constant average colour.

    This is the layout: a sidebar, a page pane and a right panel are three runs of
    columns whose mean colours differ, and the boundary between them is where the
    mean actually steps. Each band carries its own mean colour, its extent and its
    dominant colour, because the two disagree wherever a band contains a gradient
    (the reference's sidebar wash) and that disagreement is itself a measurement.
    """
    length = a.shape[1] if axis == "x" else a.shape[0]
    mean = a.mean(axis=0 if axis == "x" else 1)
    step = np.max(np.abs(np.diff(mean, axis=0)), axis=1)
    edges = [int(i) + 1 for i in range(len(step)) if step[i] >= threshold]
    out = []
    for start, end in _segments_from_edges(length, edges):
        if end - start < min_len:
            continue
        slab = a[:, start:end] if axis == "x" else a[start:end, :]
        out.append({
            "start": start,
            "end": end,
            "size": end - start,
            "mean_hex": hexof(slab.reshape(-1, 3).mean(axis=0)),
            "top": dominant(slab, n=1)[0]["hex"],
        })
    return out


def blocks(a: np.ndarray, x0: int, x1: int, y0: int, y1: int,
           background=None, tol: int = TOL, min_h: int = 4, max_h: int = 200) -> list[dict]:
    """The rows of content inside a column band -- i.e. the controls in a rail.

    Each block is a run of rows where enough pixels differ from the band's
    background, and it reports its own rectangle, its centre (what a caller
    clicks) and its horizontal extent. A rail is a column of icon plates, so this
    is what finds them without hard-coding a y for each one.
    """
    region = a[y0:y1, x0:x1]
    if background is None:
        base = bg(a, (x0, y0, x1, y1))
    else:
        base = np.asarray(background, dtype=np.int16)
    differs = (np.max(np.abs(region - base), axis=2) > tol)
    fraction = differs.mean(axis=1)
    rows = fraction >= ROW_FRACTION
    out = []
    start = None
    for i, hit in enumerate(list(rows) + [False]):
        if hit and start is None:
            start = i
        elif not hit and start is not None:
            height = i - start
            if min_h <= height <= max_h:
                slab = differs[start:i]
                cols = np.where(slab.any(axis=0))[0]
                out.append({
                    "y0": y0 + start,
                    "y1": y0 + i,
                    "h": height,
                    "x0": x0 + int(cols.min()) if len(cols) else x0,
                    "x1": x0 + int(cols.max()) + 1 if len(cols) else x1,
                    "cover": round(float(fraction[start:i].mean()), 3),
                })
            start = None
    return out


def columns(a: np.ndarray, x0: int, x1: int, y0: int, y1: int,
            background=None, tol: int = TOL, min_w: int = 4) -> list[dict]:
    """The columns of content inside a row band -- inline controls, chips, icons."""
    region = a[y0:y1, x0:x1]
    base = bg(a, (x0, y0, x1, y1)) if background is None else np.asarray(background, dtype=np.int16)
    differs = (np.max(np.abs(region - base), axis=2) > tol)
    fraction = differs.mean(axis=0)
    out = []
    start = None
    for i, hit in enumerate(list(fraction >= ROW_FRACTION) + [False]):
        if hit and start is None:
            start = i
        elif not hit and start is not None:
            if i - start >= min_w:
                out.append({"x0": x0 + start, "x1": x0 + i, "w": i - start})
            start = None
    return out


def radius(a: np.ndarray, box, fill=None, background=None) -> dict:
    """Estimate a rounded rectangle's corner radius from its own corner.

    Scans the top-left corner row by row: for each row it measures how far in the
    fill colour starts, and the largest of those insets over the arc is the
    radius. Reported with the bottom-left corner's inset as a check, because a
    square-cornered box answers 0 at both and a mis-located box answers two
    different numbers.
    """
    x0, y0, x1, y1 = box
    if fill is None:
        fill = bg(a, box)
    fill = np.asarray(fill, dtype=np.int16)
    if background is None:
        outside = np.asarray(bg(a, (max(0, x0 - 4), y0, x1 + 4, y0 + 1)), dtype=np.int16)
    else:
        outside = np.asarray(background, dtype=np.int16)

    def inset_from_top():
        worst = 0
        for y in range(y0, min(y0 + 40, y1)):
            row = a[y, x0:x1]
            hits = np.where(np.max(np.abs(row - fill), axis=1) <= TOL)[0]
            if len(hits):
                worst = max(worst, int(hits[0]))
        return worst

    def inset_from_bottom():
        worst = 0
        for y in range(max(y0, y1 - 40), y1):
            row = a[y, x0:x1]
            hits = np.where(np.max(np.abs(row - fill), axis=1) <= TOL)[0]
            if len(hits):
                worst = max(worst, int(hits[0]))
        return worst

    top = inset_from_top()
    bottom = inset_from_bottom()
    return {"radius_top": top, "radius_bottom": bottom, "agree": top == bottom,
            "fill": hexof(fill), "outside": hexof(outside)}


def text_rows(a: np.ndarray, box, threshold: int = 34) -> list[dict]:
    """The text lines inside a box, by the rows that carry ink.

    Reports each line's band height and its x-extent. Band height is a cap+descender
    measurement rather than a font size, so it is reported as measured; for Inter
    the cap height is about 0.72 em and the ascender box about 1.0, which is what
    turns a band into the `font-size` the reference set it at.
    """
    x0, y0, x1, y1 = box
    region = a[y0:y1, x0:x1]
    base = np.asarray(bg(a, box), dtype=np.int16)
    ink = (np.max(np.abs(region - base), axis=2) > threshold)
    fraction = ink.mean(axis=1)
    out = []
    start = None
    for i, hit in enumerate(list(fraction >= 0.02) + [False]):
        if hit and start is None:
            start = i
        elif not hit and start is not None:
            if i - start >= 4:
                cols = np.where(ink[start:i].any(axis=0))[0]
                out.append({
                    "y0": y0 + start,
                    "y1": y0 + i,
                    "band": i - start,
                    "x0": x0 + int(cols.min()) if len(cols) else x0,
                    "x1": x0 + int(cols.max()) + 1 if len(cols) else x1,
                })
            start = None
    return out


def ramp(a: np.ndarray, x: int, y0: int, y1: int, samples: int = 6) -> list[dict]:
    """A colour's run down one column -- how a gradient wash behaves.

    Sampled at the same places `panel_gate`'s ramp gate samples, so a new surface
    is measured the way the existing ones were.
    """
    out = []
    for i in range(samples):
        y = y0 + (y1 - y0) * i // max(1, samples - 1)
        out.append({"y": y, "hex": hexof(a[y, x])})
    return out


def ocr(path: str | Path, refresh: bool = False) -> list[dict]:
    """Every text line `tools/refocr.ps1` can read, with its box.

    Cached beside the capture as `<capture>.ocr.tsv`, because OCR costs a second
    or two and a walk asks for the same capture's labels several times. Pass
    `refresh=True` after changing the image under the same name.
    """
    tsv = Path(str(path) + ".ocr.tsv")
    if refresh or not tsv.exists():
        subprocess.run(["powershell", "-NoProfile", "-ExecutionPolicy", "Bypass",
                        "-File", str(HERE / "refocr.ps1"),
                        "-Image", str(path), "-Tsv", str(tsv)],
                       check=True, capture_output=True, text=True)
    lines = []
    for row in tsv.read_text(encoding="utf-8-sig").splitlines():
        parts = row.split("\t")
        if len(parts) < 5:
            continue
        x, y, w, h = (int(v) for v in parts[:4])
        lines.append({"x": x, "y": y, "w": w, "h": h, "text": parts[4]})
    return lines


def inside(line: dict, box) -> bool:
    """Whether a recognised line belongs to a region, judged by its centre."""
    x0, y0, x1, y1 = box
    cx = line["x"] + line["w"] / 2
    cy = line["y"] + line["h"] / 2
    return x0 <= cx < x1 and y0 <= cy < y1


def region(path: str | Path, box, refresh: bool = False) -> dict:
    """One region of a capture: its surfaces, its blocks, its lines and its text.

    This is the unit the walk works in -- "what is the rail", "what is the right
    panel", "what is a card in the middle" -- because a whole 1280x720 window
    answers a question about its layout and not about any one control.
    """
    x0, y0, x1, y1 = box
    a = array(load(path))
    return {
        "box": [x0, y0, x1, y1],
        "palette": dominant(a, box, n=8),
        "bands_x": bands(a[y0:y1, x0:x1], "x"),
        "bands_y": bands(a[y0:y1, x0:x1], "y"),
        "blocks": blocks(a, x0, x1, y0, y1),
        "text": text_rows(a, box),
        "labels": [ln for ln in ocr(path, refresh) if inside(ln, box)],
    }


def ascii_art(a: np.ndarray, box, cols: int = 110, letters: bool = True) -> str:
    """A region as text: one character per cell, for the colour it matches.

    Written because the captures cannot be looked at directly here, and a shape
    read off a list of runs is a guess while a shape read off a map is not. In
    `letters` mode each cell is the letter of the palette entry it is nearest to
    (a palette taken from the region itself, most common first, so `.` is the
    background and the rest are surfaces, plates, text and accents in the order
    they matter). Without it the cell is a luminance ramp instead, which is the
    better view of a gradient and the worse one of a card.
    """
    x0, y0, x1, y1 = box
    region = a[y0:y1, x0:x1]
    if region.size == 0:
        return ""
    # One character is one cell; the region is box-averaged down to that grid so
    # a 1px border survives as a tinted cell instead of vanishing between
    # samples.
    rows = max(1, int(round(cols * region.shape[0] / max(1, region.shape[1]) / 2.1)))
    image = Image.fromarray(region.astype(np.uint8)).resize((cols, rows), Image.BOX)
    small = np.asarray(image, dtype=np.int16)
    out = []
    if letters:
        palette = [c["rgb"] for c in dominant(region, None, n=8)]
        names = "abcdefgh"
        out.append("palette: " + ", ".join(
            f"{names[i]}={hexof(rgb)}" for i, rgb in enumerate(palette)))
        lut = np.array(palette, dtype=np.int16)
        for row in small:
            line = ""
            for px in row:
                d = np.max(np.abs(lut - px), axis=1)
                line += names[int(np.argmin(d))]
            out.append(line)
    else:
        ramp = " .:-=+*#%@"
        gray = small.mean(axis=2)
        lo, hi = float(gray.min()), float(gray.max())
        span = max(1.0, hi - lo)
        for row in gray:
            line = ""
            for value in row:
                i = int((value - lo) / span * (len(ramp) - 1))
                line += ramp[i]
            out.append(line)
    return "\n".join(out)


def report(path: str | Path) -> dict:
    """Everything above, for one capture, as one JSON-able dictionary."""
    image = load(path)
    a = array(image)
    h, w = a.shape[:2]
    horizontal = bands(a, "x")
    vertical = bands(a, "y")
    return {
        "path": str(path),
        "size": [w, h],
        "palette": dominant(a, None, n=14),
        "bands_x": horizontal,
        "bands_y": vertical,
    }


def main() -> int:
    argv = sys.argv[1:]
    if not argv:
        print(__doc__)
        return 2
    path = argv[0]
    as_json = None
    blocks_box = None
    if "--json" in argv:
        as_json = argv[argv.index("--json") + 1]
    if "--blocks" in argv:
        i = argv.index("--blocks")
        blocks_box = [int(v) for v in argv[i + 1:i + 5]]

    region_box = None
    if "--region" in argv:
        i = argv.index("--region")
        region_box = [int(v) for v in argv[i + 1:i + 5]]
    columns_box = None
    if "--columns" in argv:
        i = argv.index("--columns")
        columns_box = [int(v) for v in argv[i + 1:i + 5]]
    do_ocr = "--ocr" in argv
    refresh = "--refresh" in argv
    ascii_box = None
    if "--ascii" in argv:
        i = argv.index("--ascii")
        ascii_box = [int(v) for v in argv[i + 1:i + 5]]
    ascii_cols = 110
    if "--cols" in argv:
        ascii_cols = int(argv[argv.index("--cols") + 1])

    image = load(path)
    a = array(image)
    out = report(path)
    print(f"{path}  {out['size'][0]}x{out['size'][1]}")
    if ascii_box:
        print(ascii_art(a, ascii_box, ascii_cols, letters="--gray" not in argv))
        return 0
    if region_box:
        r = region(path, region_box, refresh)
        out["region"] = r
        print(f"region {region_box}:")
        print("  palette: " + ", ".join(f"{c['hex']}({c['coverage'] * 100:.1f}%)" for c in r["palette"]))
        print("  bands x: " + ", ".join(f"{b['start']}..{b['end']}({b['mean_hex']})" for b in r["bands_x"]))
        print("  bands y: " + ", ".join(f"{b['start']}..{b['end']}({b['mean_hex']})" for b in r["bands_y"]))
        print("  blocks (controls):")
        for b in r["blocks"]:
            print(f"    y {b['y0']:>4}..{b['y1']:<4} h={b['h']:<3} x {b['x0']:>4}..{b['x1']:<4} cover={b['cover']}")
        print("  text lines:")
        for t in r["text"]:
            print(f"    y {t['y0']:>4}..{t['y1']:<4} band={t['band']:<3} x {t['x0']:>4}..{t['x1']}")
        print("  labels:")
        for ln in r["labels"]:
            print(f"    {ln['x']:>4},{ln['y']:>4} {ln['w']:>4}x{ln['h']:<3} {ln['text']}")
        if as_json:
            Path(as_json).write_text(json.dumps(out, indent=2), encoding="utf-8")
            print(f"wrote {as_json}")
        return 0
    if columns_box:
        x0, y0, x1, y1 = columns_box
        out["columns"] = columns(a, x0, x1, y0, y1)
        print(f"columns in x[{x0}:{x1}] y[{y0}:{y1}] against {hexof(bg(a, columns_box))}:")
        for c in out["columns"]:
            print(f"  x {c['x0']:>4}..{c['x1']:<4} w={c['w']}")
        if as_json:
            Path(as_json).write_text(json.dumps(out, indent=2), encoding="utf-8")
            print(f"wrote {as_json}")
        return 0
    if do_ocr:
        out["ocr"] = ocr(path, refresh)
        print("labels:")
        for ln in out["ocr"]:
            print(f"  {ln['x']:>4},{ln['y']:>4} {ln['w']:>4}x{ln['h']:<3} {ln['text']}")
        if as_json:
            Path(as_json).write_text(json.dumps(out, indent=2), encoding="utf-8")
            print(f"wrote {as_json}")
        return 0
    if blocks_box:
        x0, y0, x1, y1 = blocks_box
        out["blocks"] = blocks(a, x0, x1, y0, y1)
        print(f"blocks in x[{x0}:{x1}] y[{y0}:{y1}]:")
        for b in out["blocks"]:
            print(f"  y {b['y0']:>4}..{b['y1']:<4} h={b['h']:<3} x {b['x0']:>4}..{b['x1']:<4} cover={b['cover']}")
    print("horizontal bands (x):")
    for b in out["bands_x"]:
        print(f"  x {b['start']:>4}..{b['end']:<4} w={b['size']:<4} mean={b['mean_hex']} top={b['top']}")
    print("vertical bands (y):")
    for b in out["bands_y"]:
        print(f"  y {b['start']:>4}..{b['end']:<4} h={b['size']:<4} mean={b['mean_hex']} top={b['top']}")
    print("palette:")
    for c in out["palette"]:
        print(f"  {c['hex']}  {c['coverage'] * 100:6.2f}%  {c['count']}")
    if as_json:
        Path(as_json).write_text(json.dumps(out, indent=2), encoding="utf-8")
        print(f"wrote {as_json}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
