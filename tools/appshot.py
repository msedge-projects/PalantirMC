"""Capture one of this launcher's own pages, without touching the desktop.

    python tools/appshot.py --page home --size 1280x720 --out shot.png
    python tools/appshot.py --exe dist/PalantirMC.exe --page home --out shot.png

Why this and not `winshot.py`: a capture of *our* window does not need a window
tool at all. `--shot` (see `PalantirApp::set_shot`) makes the launcher ask iced
for its own frame, write it as a PNG and close, so the pixels are the ones this
shell drew rather than the compositor's opinion of a window parked off the
desktop -- which is what `PrintWindow` returns, and why the same command used to
give a black image on one run and the page on the next. Nothing here activates a
window, moves the pointer or captures the screen; the window is born past the
right edge of every monitor, from the size and position in its own settings.

The launcher runs in a throwaway directory holding `portable.dat`, so a capture
reads its own empty data root instead of yours and writes its window size there
rather than into your settings. The sandbox is removed on the way out -- with a
debug build it holds a copy of a 400 MB executable, so leaving it behind is a
disk leak in a tool run once per page.

What it prints is what the gates read: the file it wrote and its size. The
capture itself is the app's business; a failure to produce one is reported with
the launcher's exit code so a caller can tell "the app died" from "the app never
took the picture".
"""
from __future__ import annotations

import argparse
import os
import shutil
import subprocess
import sys
import tempfile
import time
from pathlib import Path

# How long the launcher gets to appear, settle and write. The app's own settle
# timer is 3s (`app::SHOT_SETTLE`); the rest is startup, which on a debug build
# with a cold cache is a couple of seconds on its own.
TIMEOUT = 90.0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__,
                                     formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--exe", default="target/debug/PalantirMC.exe",
                        help="the launcher to capture (default: the debug build)")
    parser.add_argument("--page", default=None,
                        help="the page to open on, passed straight through as --page")
    parser.add_argument("--size", default="1280x720",
                        help="client size to capture at (default 1280x720, the size "
                             "the reference is measured at)")
    parser.add_argument("--out", required=True, help="where to write the PNG")
    parser.add_argument("--keep", action="store_true",
                        help="leave the throwaway directory behind, for a run that "
                             "has to be diagnosed from its own files")
    args = parser.parse_args()

    exe = Path(args.exe).resolve()
    if not exe.exists():
        print(f"no such executable: {exe}", file=sys.stderr)
        return 2
    out = Path(args.out).resolve()
    out.parent.mkdir(parents=True, exist_ok=True)
    if out.exists():
        out.unlink()

    sandbox = Path(tempfile.mkdtemp(prefix="appshot-"))
    (sandbox / "portable.dat").touch()
    copied = sandbox / exe.name
    shutil.copy2(exe, copied)

    command = [str(copied), "--shot", str(out), "--size", args.size]
    if args.page:
        command += ["--page", args.page]

    started = time.time()
    proc = subprocess.Popen(command, cwd=sandbox)
    try:
        while time.time() - started < TIMEOUT:
            if out.exists() and out.stat().st_size > 0:
                break
            if proc.poll() is not None:
                break
            time.sleep(0.25)
        # The process closes its own window once the file is down, so a short
        # grace period is what separates "done" from "wrote the file and hung".
        for _ in range(40):
            if proc.poll() is not None:
                break
            time.sleep(0.25)
    finally:
        if proc.poll() is None:
            proc.kill()
            proc.wait()
        if not args.keep:
            shutil.rmtree(sandbox, ignore_errors=True)

    code = proc.returncode
    if not out.exists() or out.stat().st_size == 0:
        print(f"no capture: the launcher exited {code} without writing {out}",
              file=sys.stderr)
        return 1

    from PIL import Image  # imported late so a missing Pillow fails after the run

    size = Image.open(out).size
    print(f"{out}: {size[0]}x{size[1]}, written in {time.time() - started:.1f}s "
          f"(launcher exit {code})")
    if size != tuple(int(v) for v in args.size.split("x")):
        print(f"the capture is {size[0]}x{size[1]}, not the {args.size} asked for -- "
              f"every number a gate measures off it would be wrong by a scale factor",
              file=sys.stderr)
        return 1
    if code not in (0, None):
        print(f"warning: the launcher exited {code}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
