#!/usr/bin/env python3
"""Throwaway probe: open Preferences > Java and dump the OCR words."""
import os, subprocess, sys, tempfile, time
from pathlib import Path

def command(*args):
    return subprocess.run(args, check=True, text=True, stdout=subprocess.PIPE,
                          stderr=subprocess.PIPE).stdout

binary = Path(sys.argv[1]).resolve(strict=True)
with tempfile.TemporaryDirectory(prefix="palantir-probe-") as temp:
    root = Path(temp)
    window = None
    log = (root / "launcher.log").open("w")
    app = subprocess.Popen([str(binary)], env={**os.environ, "PALANTIR_DATA_DIR": str(root / "data")},
                           stdout=log, stderr=log)
    try:
        deadline = time.monotonic() + 15
        while time.monotonic() < deadline and window is None:
            for probe in (["xdotool", "search", "--pid", str(app.pid)],
                          ["xdotool", "search", "--name", "PalantirMC"]):
                found = subprocess.run(probe, text=True, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
                if found.returncode == 0 and found.stdout.strip():
                    window = found.stdout.splitlines()[-1]
                    break
            if window is None:
                time.sleep(.1)
        subprocess.run(["xdotool", "windowfocus", window], check=True)
        subprocess.run(["xdotool", "windowsize", window, "1280", "800"], check=True)
        time.sleep(.8)
        subprocess.run(["xdotool", "key", "ctrl+comma"], check=True)
        time.sleep(.8)
        # Click the Java tab by its coordinates from OCR.
        png = root / "shot.png"
        command("import", "-window", window, str(png))
        scaled = root / "shot-3x.png"
        command("convert", str(png), "-resize", "300%", str(scaled))
        output = command("tesseract", str(scaled), "stdout", "tsv")
        for line in output.splitlines()[1:]:
            fields = line.split("\t", 11)
            if len(fields) != 12:
                continue
            text = fields[11].strip()
            if text and text.lower() in ("java", "java&player", "add", "executable…"):
                left, top, w, h = (int(fields[i]) for i in (6, 7, 8, 9))
                print(f"CANDIDATE {text!r} at ({left//3 + w//6},{top//3 + h//6})")
                subprocess.run(["xdotool", "mousemove", "--window", window,
                                str(left//3 + w//6), str(top//3 + h//6)], check=True)
                subprocess.run(["xdotool", "click", "1"], check=True)
                time.sleep(.6)
                png2 = root / "shot2.png"
                command("import", "-window", window, str(png2))
                scaled2 = root / "shot2-3x.png"
                command("convert", str(png2), "-resize", "300%", str(scaled2))
                out2 = command("tesseract", str(scaled2), "stdout", "tsv")
                seen = []
                for line2 in out2.splitlines()[1:]:
                    f2 = line2.split("\t", 11)
                    if len(f2) == 12 and f2[11].strip():
                        seen.append((int(f2[7]) // 3, f2[11].strip()))
                seen.sort()
                print("WORDS AFTER CLICK:", [t for _, t in seen])
                raise SystemExit(0)
        print("no java tab candidate found")
    finally:
        app.terminate()
        app.wait(timeout=10)
