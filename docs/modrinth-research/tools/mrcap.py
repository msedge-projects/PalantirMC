#!/usr/bin/env python3
"""Capture and drive the Modrinth App on a private Xvfb display.

Research tooling for docs/modrinth-research. Nothing here is shipped: it only
screenshot/OCR-clicks a running window so claims in the modules can carry
OBSERVED evidence ids. Same discipline as rewrite/tools/ui_smoke.py -- we only
ever click a control we have actually OCR'd and read, never a coordinate we
assumed.

Usage:
    mrcap.py shot   S-0001 [1280x800] [--out DIR]   capture + ocr the window
    mrcap.py ocr    PATH                            dump observed controls
    mrcap.py click  "exact text" [occurrence]       click an observed control
    mrcap.py key    KEY...                          send keys to the window
    mrcap.py size   WxH                             resize the window
    mrcap.py find   "substring"                     list matching observed controls
"""
import argparse
import json
import os
import subprocess
import sys
import time
from pathlib import Path

DEFAULT_TITLE = "Modrinth App"
RESEARCH = Path(__file__).resolve().parent.parent
SHOTS = RESEARCH / "screenshots"


def run(*args, check=True):
    return subprocess.run(args, check=check, text=True,
                          stdout=subprocess.PIPE, stderr=subprocess.PIPE).stdout


def window(display=None):
    display = display or os.environ["DISPLAY"]
    out = run("xdotool", "search", "--onlyvisible", "--name", f"^{DEFAULT_TITLE}$")
    ids = [line.strip() for line in out.splitlines() if line.strip()]
    if not ids:
        raise SystemExit(f"no window titled {DEFAULT_TITLE!r} on {display}")
    return ids[-1]


def focus(wid):
    subprocess.run(["xdotool", "windowactivate", "--sync", wid],
                   stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    time.sleep(0.4)


def size(wid, geometry):
    w, h = geometry.lower().split("x")
    run("xdotool", "windowsize", wid, w, h)
    time.sleep(1.2)  # webview reflows; capture only once it has settled


def capture(wid, path):
    path.parent.mkdir(parents=True, exist_ok=True)
    run("import", "-window", wid, str(path))
    return path


def ocr(path, psm="11"):
    """Return observed controls: [{text, x, y, w, h, conf}] in window coords."""
    tsv = run("tesseract", str(path), "stdout", "--psm", psm, "tsv")
    found = []
    for line in tsv.splitlines()[1:]:
        f = line.split("\t", 11)
        if len(f) != 12:
            continue
        try:
            left, top, w, h, conf = (int(f[6]), int(f[7]), int(f[8]), int(f[9]), float(f[10]))
        except ValueError:
            continue
        text = f[11].strip()
        if text and conf > 40:
            found.append({"text": text, "x": left + w // 2, "y": top + h // 2,
                          "w": w, "h": h, "conf": round(conf, 1)})
    return found


def click_text(wid, needle, occurrence=1, psm="11"):
    """Click the nth OCR'd control containing needle. Observed-only by design."""
    path = Path(os.environ.get("MRCAP_TMP", "/tmp/mrcap.png"))
    capture(wid, path)
    hits = [c for c in ocr(path, psm) if needle.lower() in c["text"].lower()]
    if not hits:
        print(json.dumps({"clicked": False, "reason": "not observed",
                          "needle": needle,
                          "saw": [c["text"] for c in ocr(path, psm)]}, indent=2))
        return 1
    if occurrence > len(hits):
        print(json.dumps({"clicked": False, "reason": "occurrence out of range",
                          "needle": needle, "matches": len(hits)}, indent=2))
        return 1
    c = hits[occurrence - 1]
    run("xdotool", "mousemove", "--window", wid, str(c["x"]), str(c["y"]))
    time.sleep(0.3)
    run("xdotool", "click", "1")
    time.sleep(1.0)
    print(json.dumps({"clicked": True, "needle": needle, "at": [c["x"], c["y"]],
                      "text": c["text"]}, indent=2))
    return 0


def main():
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="cmd", required=True)

    p = sub.add_parser("shot"); p.add_argument("id"); p.add_argument("geometry", nargs="?", default="1280x800")
    p.add_argument("--out", default=str(SHOTS)); p.add_argument("--psm", default="11"); p.add_argument("--no-ocr", action="store_true")

    p = sub.add_parser("ocr"); p.add_argument("path")

    p = sub.add_parser("click"); p.add_argument("text"); p.add_argument("occurrence", nargs="?", type=int, default=1)
    p.add_argument("--psm", default="11")

    p = sub.add_parser("key"); p.add_argument("keys", nargs="+")

    p = sub.add_parser("size"); p.add_argument("geometry")

    p = sub.add_parser("find"); p.add_argument("needle"); p.add_argument("--psm", default="11")

    p = sub.add_parser("hover"); p.add_argument("text"); p.add_argument("occurrence", nargs="?", type=int, default=1)
    p.add_argument("--psm", default="11")

    p = sub.add_parser("rail"); p.add_argument("--psm", default="11")

    args = ap.parse_args()
    wid = window() if args.cmd != "ocr" else None

    if args.cmd == "shot":
        focus(wid)
        if args.geometry:
            size(wid, args.geometry)
        out = Path(args.out) / f"{args.id}.png"
        capture(wid, out)
        result = {"id": args.id, "file": str(out), "geometry": args.geometry}
        if not args.no_ocr:
            result["observed"] = ocr(out, args.psm)
        print(json.dumps(result, indent=2))
    elif args.cmd == "ocr":
        print(json.dumps(ocr(Path(args.path)), indent=2))
    elif args.cmd == "click":
        focus(wid)
        return click_text(wid, args.text, args.occurrence, args.psm)
    elif args.cmd == "key":
        focus(wid)
        for k in args.keys:
            run("xdotool", "key", "--clearmodifiers", k)
            time.sleep(0.6)
    elif args.cmd == "size":
        size(wid, args.geometry)
    elif args.cmd == "find":
        focus(wid)
        p = Path(os.environ.get("MRCAP_TMP", "/tmp/mrcap.png"))
        capture(wid, p)
        print(json.dumps([c for c in ocr(p, args.psm)
                          if args.needle.lower() in c["text"].lower()], indent=2))
    elif args.cmd == "hover":
        # Icon-only rail buttons carry no OCR text of their own, so the tooltip
        # is the observation: hover, wait, read what the app drew.
        focus(wid)
        p = Path(os.environ.get("MRCAP_TMP", "/tmp/mrcap.png"))
        capture(wid, p)
        hits = [c for c in ocr(p, args.psm) if args.text.lower() in c["text"].lower()]
        if not hits:
            print(json.dumps({"hovered": False, "needle": args.text}, indent=2))
            return 1
        c = hits[args.occurrence - 1]
        run("xdotool", "mousemove", "--window", wid, str(c["x"]), str(c["y"]))
        time.sleep(1.8)  # webview tooltip fade-in
        print(json.dumps({"hovered": True, "over": [c["x"], c["y"]],
                          "text": c["text"],
                          "tip": ocr(p, args.psm)}, indent=2))
    elif args.cmd == "rail":
        # Walk the left rail top to bottom, hover each slot and OCR whatever
        # label appears. Used once to learn the nav tree; afterwards the
        # labels found here are what every other worker clicks by name.
        focus(wid)
        p = Path(os.environ.get("MRCAP_TMP", "/tmp/mrcap.png"))
        capture(wid, p)
        height = int(run("xdotool", "getwindowgeometry", "--shell", wid)
                     .split("HEIGHT=")[1].split("\n")[0])
        rows = []
        y = 56
        while y < height - 40:
            run("xdotool", "mousemove", "--window", wid, "32", str(y))
            time.sleep(0.9)
            capture(wid, p)
            rows.append({"y": y, "texts": [c["text"] for c in ocr(p, args.psm)]})
            y += 48
        print(json.dumps(rows, indent=2))
    return 0


if __name__ == "__main__":
    sys.exit(main())