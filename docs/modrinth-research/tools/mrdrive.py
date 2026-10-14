#!/usr/bin/env python3
"""Drive the Modrinth App by OCR-observed controls, phrase-aware.

Same discipline as `mrcap.py` (research docs/modrinth-research): we only ever
act on a control we have read on screen, never on a coordinate we assumed.

Why this exists: `mrcap.py click` matches a single OCR token. Real labels are
phrases ("Create an instance", "Import from launcher"), and OCR returns each
word separately, so a phrase needle never matches. This groups OCR words into
visual lines and matches the whole phrase against the line, clicking the centre
of the words that the phrase actually covers.

Usage:
    mrdrive.py shot ID [WxH]        capture the window -> screenshots/ID.png
    mrdrive.py text                 print observed lines as "x,y  <text>"
    mrdrive.py click "phrase" [n]   click the nth line containing phrase
    mrdrive.py rightclick "phrase" [n]
    mrdrive.py hover "phrase" [n]
    mrdrive.py move X Y             move the pointer (no click)
    mrdrive.py clickat X Y          left-click a read-off coordinate (green-on-green labels)
    mrdrive.py rclickat X Y         right-click at a read-off coordinate (context menus)
    mrdrive.py key KEY...           send keys (xdotool names)
    mrdrive.py type TEXT            type text
    mrdrive.py wheel N              scroll N notches (positive = down)
    mrdrive.py size WxH             resize the window
"""
import argparse
import json
import os
import subprocess
import sys
import time
from pathlib import Path

TITLE = "Modrinth App"
RESEARCH = Path(__file__).resolve().parent.parent
SHOTS = RESEARCH / "screenshots"
TMP = Path(os.environ.get("MRDRIVE_TMP", "/tmp/mrdrive"))
TMP.mkdir(parents=True, exist_ok=True)
CUR = TMP / "cur.png"


def run(*args, check=True):
    return subprocess.run(args, check=check, text=True,
                          stdout=subprocess.PIPE, stderr=subprocess.PIPE).stdout


def window():
    out = run("xdotool", "search", "--onlyvisible", "--name", f"^{TITLE}$")
    ids = [l.strip() for l in out.splitlines() if l.strip()]
    if not ids:
        raise SystemExit(f"no window titled {TITLE!r} on {os.environ.get('DISPLAY')}")
    return ids[-1]


def focus(wid):
    subprocess.run(["xdotool", "windowactivate", "--sync", wid],
                   stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    time.sleep(0.35)


def capture(wid, path=CUR, settle=0.0):
    if settle:
        time.sleep(settle)
    path.parent.mkdir(parents=True, exist_ok=True)
    run("import", "-window", wid, str(path))
    return path


def lines(path, psm="11"):
    """OCR -> visual lines. Each line: {text, x, y, words:[{text,x,y,w,h}]}."""
    tsv = run("tesseract", str(path), "stdout", "--psm", psm, "tsv")
    buckets = {}
    for line in tsv.splitlines()[1:]:
        f = line.split("\t", 11)
        if len(f) != 12:
            continue
        try:
            block, par, lineno = int(f[2]), int(f[3]), int(f[4])
            left, top, w, h, conf = int(f[6]), int(f[7]), int(f[8]), int(f[9]), float(f[10])
        except ValueError:
            continue
        text = f[11].strip()
        if not text or conf <= 40:
            continue
        key = (block, par, lineno)
        buckets.setdefault(key, []).append(
            {"text": text, "x": left + w // 2, "y": top + h // 2,
             "l": left, "t": top, "w": w, "h": h})
    out = []
    for key in sorted(buckets):
        ws = sorted(buckets[key], key=lambda w: w["l"])
        l = min(w["l"] for w in ws)
        t = min(w["t"] for w in ws)
        r = max(w["l"] + w["w"] for w in ws)
        b = max(w["t"] + w["h"] for w in ws)
        out.append({"text": " ".join(w["text"] for w in ws),
                    "x": (l + r) // 2, "y": (t + b) // 2, "words": ws})
    return out


def find(lines_, phrase, nth=1):
    hits = [ln for ln in lines_ if phrase.lower() in ln["text"].lower()]
    if nth > len(hits):
        return None
    return hits[nth - 1] if hits else None


def phrase_center(ln, phrase):
    """Centre of the words the phrase covers (fall back to whole line)."""
    words = ln["words"]
    joined = ln["text"].lower()
    idx = joined.find(phrase.lower())
    if idx < 0:
        return ln["x"], ln["y"]
    # Walk words accumulating characters until the phrase span is covered.
    pos = 0
    start = end = None
    for i, w in enumerate(words):
        if start is None and pos >= idx:
            start = i
        pos += len(w["text"]) + 1
        if end is None and pos - 1 >= idx + len(phrase):
            end = i
    if start is None:
        return ln["x"], ln["y"]
    end = end if end is not None else len(words) - 1
    cov = words[start:end + 1]
    l = min(w["l"] for w in cov); r = max(w["l"] + w["w"] for w in cov)
    t = min(w["t"] for w in cov); b = max(w["t"] + w["h"] for w in cov)
    return (l + r) // 2, (t + b) // 2


def act(wid, phrase, nth, button="1"):
    capture(wid, settle=0.15)
    ln = find(lines(CUR), phrase, nth)
    if not ln:
        seen = [l["text"] for l in lines(CUR)]
        print(json.dumps({"acted": False, "reason": "not observed",
                          "phrase": phrase, "saw": seen}, indent=2))
        return 1
    x, y = phrase_center(ln, phrase)
    run("xdotool", "mousemove", "--window", wid, str(x), str(y))
    time.sleep(0.25)
    if button == "hover":
        time.sleep(1.6)
        print(json.dumps({"hovered": True, "phrase": phrase, "at": [x, y],
                          "tip": [l["text"] for l in lines(CUR)]}, indent=2))
        return 0
    run("xdotool", "click", button)
    time.sleep(1.1)
    print(json.dumps({"acted": True, "phrase": phrase, "at": [x, y],
                      "line": ln["text"]}, indent=2))
    return 0


def main():
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="cmd", required=True)
    p = sub.add_parser("shot"); p.add_argument("id"); p.add_argument("geometry", nargs="?", default=None)
    sub.add_parser("text")
    p = sub.add_parser("click"); p.add_argument("phrase"); p.add_argument("n", nargs="?", type=int, default=1)
    p = sub.add_parser("rightclick"); p.add_argument("phrase"); p.add_argument("n", nargs="?", type=int, default=1)
    p = sub.add_parser("hover"); p.add_argument("phrase"); p.add_argument("n", nargs="?", type=int, default=1)
    p = sub.add_parser("move"); p.add_argument("x", type=int); p.add_argument("y", type=int)
    p = sub.add_parser("clickat"); p.add_argument("x", type=int); p.add_argument("y", type=int)
    p = sub.add_parser("rclickat"); p.add_argument("x", type=int); p.add_argument("y", type=int)
    p = sub.add_parser("key"); p.add_argument("keys", nargs="+")
    p = sub.add_parser("type"); p.add_argument("text")
    p = sub.add_parser("wheel"); p.add_argument("n", type=int)
    p = sub.add_parser("size"); p.add_argument("geometry")
    args = ap.parse_args()

    wid = window()
    if args.cmd == "shot":
        focus(wid)
        if args.geometry:
            w, h = args.geometry.lower().split("x")
            run("xdotool", "windowsize", wid, w, h); time.sleep(1.3)
        out = SHOTS / f"{args.id}.png"
        capture(wid, out, settle=0.5)
        observed = lines(out)
        (TMP / f"{args.id}.ocr.json").write_text(
            json.dumps({"id": args.id, "geometry": args.geometry, "lines": observed}, indent=2))
        print(json.dumps({"id": args.id, "file": str(out),
                          "lines": [l["text"] for l in observed]}, indent=2))
    elif args.cmd == "text":
        focus(wid); capture(wid, settle=0.15)
        for l in lines(CUR):
            print(f"{l['x']:4},{l['y']:4}  {l['text']}")
    elif args.cmd in ("click", "rightclick"):
        return act(wid, args.phrase, args.n, "3" if args.cmd == "rightclick" else "1")
    elif args.cmd == "hover":
        return act(wid, args.phrase, args.n, "hover")
    elif args.cmd == "move":
        run("xdotool", "mousemove", "--window", wid, str(args.x), str(args.y)); time.sleep(0.2)
    elif args.cmd == "clickat":
        # For controls whose label OCR cannot read (e.g. green text on a green
        # button). The position is read off a screenshot that was viewed, and
        # logged, never guessed.
        run("xdotool", "mousemove", "--window", wid, str(args.x), str(args.y)); time.sleep(0.25)
        run("xdotool", "click", "1"); time.sleep(1.1)
    elif args.cmd == "rclickat":
        # Right-click at a coordinate read off a viewed screenshot. Needed for
        # context menus, which only a button-3 press opens. Coordinates are
        # window-relative, exactly like clickat.
        run("xdotool", "mousemove", "--window", wid, str(args.x), str(args.y)); time.sleep(0.25)
        run("xdotool", "click", "3"); time.sleep(1.1)
    elif args.cmd == "key":
        focus(wid)
        for k in args.keys:
            run("xdotool", "key", "--clearmodifiers", k); time.sleep(0.5)
    elif args.cmd == "type":
        focus(wid); run("xdotool", "type", "--clearmodifiers", "--delay", "40", args.text); time.sleep(0.6)
    elif args.cmd == "wheel":
        focus(wid)
        for _ in range(abs(args.n)):
            run("xdotool", "click", "5" if args.n > 0 else "4"); time.sleep(0.25)
    elif args.cmd == "size":
        w, h = args.geometry.lower().split("x")
        run("xdotool", "windowsize", wid, w, h); time.sleep(1.3)
    return 0


if __name__ == "__main__":
    sys.exit(main())
