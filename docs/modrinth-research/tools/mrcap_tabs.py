#!/usr/bin/env python3
"""Walk the Modrinth App's left navigation rail and capture each top-level tab.

Grounding rule (mission §4): never click an assumed coordinate. The rail is
icon-only, so every slot is *hovered first*, the tooltip the app draws is
OCRed and read, and only then is that same slot clicked. The tooltip name
becomes the tab's identity in the module docs and in MANIFEST.md.

Writes screenshots/S-0xxx.png and prints the mapping so the orchestrator can
transcribe it straight into INDEX.md.
"""
import json
import os
import subprocess
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from mrcap import capture, focus, ocr, run, size, window  # noqa: E402

SHOTS = Path(__file__).resolve().parent.parent / "screenshots"
BASE = [{"y": y} for y in (72, 124, 176, 228, 280, 356, 716, 768)]
BASE_TEXT = {
    "home", "offline", "running", "getting", "started", "discover", "content",
    "skin", "selector", "screenshots", "sign", "in", "to", "minecraft",
    "modrinth", "news", "create", "first", "instance", "no", "instances",
}


def tooltip_for(wid, slot, probe):
    """Hover a rail slot and return tooltip words the baseline did not have."""
    capture(wid, probe)
    before = {c["text"] for c in ocr(probe)}
    run("xdotool", "mousemove", "--window", wid, "32", str(slot))
    time.sleep(1.9)
    capture(wid, probe)
    new = [c["text"] for c in ocr(probe) if c["text"] not in before]
    # The tooltip sits right of the rail; keep only words in that strip.
    run("xdotool", "mousemove", "--window", wid, "640", "400")
    time.sleep(0.8)
    return [t for t in new if t.lower() not in BASE_TEXT]


def main():
    start = int(sys.argv[1]) if len(sys.argv) > 1 else 1
    geom = sys.argv[2] if len(sys.argv) > 2 else "1280x800"
    slots = [int(s) for s in sys.argv[3].split(",")] if len(sys.argv) > 3 else \
        [b["y"] for b in BASE]

    wid = window()
    focus(wid)
    size(wid, geom)
    probe = Path("/tmp/mrcap_rail.png")
    results = []

    for slot in slots:
        tip = tooltip_for(wid, slot, probe)
        name = " ".join(tip).strip() or f"slot{slot}"
        run("xdotool", "mousemove", "--window", wid, "32", str(slot))
        time.sleep(0.4)
        run("xdotool", "click", "1")
        time.sleep(3.0)  # webview route change + settle
        sid = f"S-{start + len(results):04d}"
        out = SHOTS / f"{sid}.png"
        capture(wid, out)
        observed = ocr(out)
        results.append({"id": sid, "slot_y": slot, "tooltip": name,
                        "words": [c["text"] for c in observed][:28]})
        print(json.dumps(results[-1]), flush=True)

    Path("/tmp/mrcap_tabs.json").write_text(json.dumps(results, indent=2))


if __name__ == "__main__":
    main()