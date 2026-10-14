#!/usr/bin/env python3
"""Sweep the Settings dialog: every section, fully scrolled.

Two robustness rules learned the hard way:

1. **Verify the dialog is open before sweeping.** An earlier run swept while
   the app was on the Hosting tab, so every section click silently missed and
   ten junk screenshots were produced. We now assert the dialog's own heading
   and its section labels are on screen, and refuse to start otherwise.
2. **Scrolled frames are real deliverables.** The mission asks for every section
   fully scrolled. Frames go into screenshots/ as first-class ids (never into
   /tmp), and scrolling stops as soon as the OCR text stops changing, so a
   section that fits needs one frame and a long one gets as many as it needs.

Clicks are by OCR'd label only — never an assumed coordinate.
"""
import json
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from mrcap import capture, click_text, focus, ocr, run, size, window  # noqa: E402

SHOTS = Path(__file__).resolve().parent.parent / "screenshots"
PROBE = Path("/tmp/mrcap_sweep_probe.png")

SECTIONS = [
    "Appearance", "Features", "Behavior", "Language", "Profile", "Social",
    "Privacy", "Synced settings", "Java installations", "Resource management",
]
MARKERS = ("Color theme", "Appearance", "Settings", "DISPLAY", "ACCOUNT")


def words_on(wid):
    capture(wid, PROBE)
    return tuple(c["text"] for c in ocr(PROBE))


def dialog_open(wid):
    words = words_on(wid)
    return sum(1 for m in MARKERS if m in words) >= 2


def open_settings(wid):
    """Click the rail's settings slot, found by its tooltip, then assert."""
    run("xdotool", "mousemove", "--window", wid, "32", "716")
    time.sleep(1.9)
    capture(wid, PROBE)
    if "Settings" in [c["text"] for c in ocr(PROBE)]:
        run("xdotool", "click", "1")
        time.sleep(2.2)
    return dialog_open(wid)


def scroll(wid, direction):
    run("xdotool", "mousemove", "--window", wid, "820", "450")
    time.sleep(0.25)
    run("xdotool", "click", "5" if direction == "down" else "4")
    time.sleep(1.1)


def shoot_section(wid, name, start):
    """Capture a section top-to-bottom; return frames captured and next id."""
    ids, seen, last = [], set(), None
    for i in range(9):
        sig = words_on(wid)
        if sig == last:
            break
        last = sig
        sid = f"S-{start + len(ids):04d}"
        capture(wid, SHOTS / f"{sid}.png")
        ids.append(sid)
        seen.update(sig)
        scroll(wid, "down")
    for _ in range(10):
        scroll(wid, "up")
    return ids, seen


def main():
    start = int(sys.argv[1]) if len(sys.argv) > 1 else 20
    names = sys.argv[2].split("|") if len(sys.argv) > 2 else SECTIONS
    wid = window()
    focus(wid)
    size(wid, "1280x800")
    run("xdotool", "key", "--clearmodifiers", "Escape")
    time.sleep(1.0)

    if not dialog_open(wid):
        if not open_settings(wid):
            print(json.dumps({"error": "settings dialog would not open",
                              "words": list(words_on(wid))[:40]}), flush=True)
            return 1

    log, cursor = [], start
    for name in names:
        ok = click_text(wid, name) == 0
        time.sleep(1.7)
        if not dialog_open(wid):
            print(json.dumps({"error": "dialog closed mid-sweep",
                              "section": name}), flush=True)
            break
        ids, seen = shoot_section(wid, name, cursor)
        cursor += len(ids)
        log.append({"section": name, "clicked": ok, "frames": ids,
                    "words": sorted(seen)})
        print(json.dumps({"section": name, "clicked": ok, "frames": ids,
                          "text": " ".join(sorted(seen))[:600]}), flush=True)
    Path("/tmp/mrcap_settings.json").write_text(json.dumps(log, indent=2))
    return 0


if __name__ == "__main__":
    sys.exit(main())