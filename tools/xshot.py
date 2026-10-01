#!/usr/bin/env python3
"""Capture and drive a window on X11 — the Linux counterpart of `winshot.py`.

Why this exists: `winshot.py`, `refwalk.py` and `appshot.py` are the Windows
capture path. They reach `user32` through `ctypes`, and three gates in `GATES.md`
(G135, G138, G141) name the consequence in their own words — no capture of the
shaped text, no reading of the layout nodes back, no picture of the dialog after
the change. On a Linux box the same job is `xdotool` for input and `ffmpeg`'s
`x11grab` for pixels, and it works with no screen at all: an `Xvfb` display is a
real X server as far as every one of these calls is concerned.

    Xvfb :99 -screen 0 1920x1080x24 &
    export DISPLAY=:99

    python tools/xshot.py --launch target/release/PalantirMC --client 1280x720 --out ours.png
    python tools/xshot.py --title "Modrinth App" --client 1280x720 --out reference.png
    python tools/xshot.py --title PalantirMC --script session.txt

Script lines are the same three `winshot.py` takes, so a session recorded there is
replayable here: `click X Y`, `wait SECONDS`, `shot OUT.png`.

**This file has never been run.** It was written on the Windows machine, where
none of it can execute, so the first session on the Ubuntu box should treat it as a
draft: run it once, fix what is wrong, commit that fix. What it assumes, in the
order those assumptions can bite:

* `$DISPLAY` names a server that is actually running (`xvfb-run` wraps a single
  command; a long-lived `Xvfb :99 &` is what a session with several captures wants).
* `--client WxH` is exact because this launcher's window is undecorated: with no
  window manager and no frame, the window's geometry *is* its client area, which is
  the whole reason `winshot.py` speaks of a "client" size and this mirrors it. Under
  a window manager (`openbox` is installed by `tools/vps_setup.sh`) the reference's
  window does get a frame, and then `--client` pins the window including it —
  capture-and-compare sessions should either pin the size and compare the whole
  window, or run with no manager at all.
* `ffmpeg` is built with `x11grab`, which is not universal: `ffmpeg -devices` says.
  Where it is missing, `import -window <id> out.png` (ImageMagick, also installed by
  the setup script) is the fallback and a one-line change here.
"""

import argparse
import os
import shlex
import shutil
import subprocess
import sys
import tempfile
import time

# What every path here needs. The message names the setup script, because a machine
# that is missing these is a machine that has not been bootstrapped yet, and saying
# so saves a debugging session about `$DISPLAY`.
REQUIRED = ("xdotool", "ffmpeg")


def require(tools=REQUIRED):
    missing = [tool for tool in tools if not shutil.which(tool)]
    if missing:
        raise SystemExit(
            f"missing {', '.join(missing)} -- run `bash tools/vps_setup.sh`. "
            "(On Windows the capture path is `python tools/winshot.py`.)"
        )


def display():
    """The X display to draw in and read from."""
    value = os.environ.get("DISPLAY")
    if not value:
        raise SystemExit(
            "DISPLAY is unset: start a display and point at it, e.g.\n"
            "  Xvfb :99 -screen 0 1920x1080x24 &\n"
            "  export DISPLAY=:99"
        )
    return value


def run(cmd, **kw):
    return subprocess.run(cmd, check=True, text=True, capture_output=True, **kw)


def xdotool(*args):
    """`xdotool`, raising on failure — for calls whose failure is a bug."""
    return run(["xdotool", *args]).stdout.strip()


def xdotool_ok(*args):
    """`xdotool`, answering empty on failure — for a search that may find nothing."""
    done = subprocess.run(["xdotool", *args], text=True, capture_output=True)
    return done.stdout.strip() if done.returncode == 0 else ""


def find_window(pid=None, title=None, timeout=25.0):
    """The id of the window to work on, waiting for it to exist.

    The *last* match rather than the first, which is the same choice `winshot.py`
    makes: a toolkit can own more than one top-level window (a splash, an input-only
    helper), and the one that is drawn last is the one on screen. `--onlyvisible`
    keeps an unmapped one out of the answer.
    """
    selector = []
    if pid is not None:
        selector += ["--pid", str(pid)]
    if title is not None:
        selector += ["--name", title]
    if not selector:
        raise SystemExit("nothing to search for: pass --title, --process or --launch")
    deadline = time.time() + timeout
    while time.time() < deadline:
        found = xdotool_ok("search", "--onlyvisible", *selector)
        if found:
            return found.splitlines()[-1]
        time.sleep(0.25)
    raise SystemExit(f"no visible window matching {' '.join(selector)} after {timeout:.0f}s")


def geometry(wid):
    """`(x, y, width, height)` of the window, in screen pixels."""
    fields = {}
    for line in xdotool("getwindowgeometry", "--shell", wid).splitlines():
        if "=" in line:
            name, value = line.split("=", 1)
            fields[name.strip()] = value.strip()
    return (
        int(fields["X"]),
        int(fields["Y"]),
        int(fields["WIDTH"]),
        int(fields["HEIGHT"]),
    )


def pin_client(wid, size):
    """Make the window exactly `WxH`, so two captures compare 1:1.

    An undecorated window has no frame to subtract, so this is the size asked for
    rather than a size solved for (which is what `winshot.py` has to do on Windows,
    where a frame's borders differ on all four edges).
    """
    if size is None:
        return
    width, height = size
    xdotool("windowsize", "--sync", wid, str(width), str(height))
    time.sleep(0.4)
    _, _, got_w, got_h = geometry(wid)
    if (got_w, got_h) != (width, height):
        print(f"note: asked for {width}x{height}, window is {got_w}x{got_h}", file=sys.stderr)


def grab(wid, out, size=None):
    """Write one PNG of the window's pixels."""
    x, y, width, height = geometry(wid)
    if size is not None:
        width, height = size
    subprocess.run(
        [
            "ffmpeg", "-y", "-v", "error",
            "-f", "x11grab",
            "-video_size", f"{width}x{height}",
            "-i", f"{display()}+{x},{y}",
            "-frames:v", "1",
            out,
        ],
        check=True,
    )
    print(f"wrote {out} ({width}x{height} from {display()}+{x},{y})")


def click(wid, x, y, settle=1.2):
    """Click at client-area `(x, y)` and let the interface settle.

    `--window` on `mousemove` is what makes the coordinates client-area ones: the
    pointer is placed relative to the window rather than to the screen, the same
    convention `winshot.py`'s `--click` uses.
    """
    xdotool("mousemove", "--window", wid, str(x), str(y))
    xdotool("click", "1")
    time.sleep(settle)


def run_script(wid, path, settle):
    """Play a session file: `click X Y`, `wait SECONDS`, `shot OUT.png`."""
    with open(path, encoding="utf-8") as script:
        for number, line in enumerate(script, start=1):
            parts = line.split("#", 1)[0].split()
            if not parts:
                continue
            verb, rest = parts[0], parts[1:]
            if verb == "click":
                click(wid, int(rest[0]), int(rest[1]), settle)
            elif verb == "wait":
                time.sleep(float(rest[0]))
            elif verb == "shot":
                grab(wid, rest[0])
            else:
                raise SystemExit(f"{path}:{number}: unknown line {line.strip()!r}")


def main():
    parser = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    parser.add_argument("--launch", help="executable to start")
    parser.add_argument("--args", default="", help="arguments for it, shell-split")
    parser.add_argument("--cwd", help="working directory for the launch")
    parser.add_argument("--portable", action="store_true",
                        help="launch in a throwaway directory holding portable.dat")
    parser.add_argument("--title", help="attach to a window whose title contains this")
    parser.add_argument("--process", help="attach to a running process by image name")
    parser.add_argument("--client", help="pin the window to WxH before capturing")
    parser.add_argument("--settle", type=float, default=6.0,
                        help="seconds to wait after the window appears (default 6)")
    parser.add_argument("--script", help="file of `click`, `wait` and `shot` lines")
    parser.add_argument("--click-settle", type=float, default=1.2)
    parser.add_argument("--out", help="where to write the single capture")
    parser.add_argument("--keep", action="store_true", help="leave a launched process running")
    args = parser.parse_args()

    if not args.out and not args.script:
        parser.error("one of --out or --script is required")
    require()
    os.environ["DISPLAY"] = display()

    size = None
    if args.client:
        width, height = args.client.lower().split("x", 1)
        size = (int(width), int(height))

    proc = None
    sandbox = None
    pid = None
    if args.launch:
        exe = os.path.abspath(args.launch)
        cwd = args.cwd
        if args.portable:
            # The marker is read *next to the executable*, not in the working
            # directory (`palantir_core::paths::portable_dir`), so the sandbox needs
            # a copy of the binary inside it rather than merely a different cwd. That
            # lesson is `winshot.py`'s, measured the wrong way round once.
            sandbox = tempfile.mkdtemp(prefix="xshot-")
            open(os.path.join(sandbox, "portable.dat"), "w", encoding="utf-8").close()
            target = os.path.join(sandbox, os.path.basename(exe))
            shutil.copy2(exe, target)
            exe, cwd = target, sandbox
        proc = subprocess.Popen([exe, *shlex.split(args.args)], cwd=cwd)
        pid = proc.pid
    elif args.process:
        found = subprocess.run(["pgrep", "-f", args.process], text=True, capture_output=True)
        if found.returncode != 0 or not found.stdout.strip():
            raise SystemExit(f"no running process matching {args.process}")
        pid = int(found.stdout.split()[0])

    try:
        wid = find_window(pid=pid, title=args.title)
        pin_client(wid, size)
        time.sleep(args.settle)
        if args.script:
            run_script(wid, args.script, args.click_settle)
        if args.out:
            grab(wid, args.out, size)
    finally:
        if proc is not None and not args.keep:
            proc.terminate()
            try:
                proc.wait(timeout=10)
            except subprocess.TimeoutExpired:
                proc.kill()
    return 0


if __name__ == "__main__":
    sys.exit(main())
