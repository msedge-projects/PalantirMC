"""Capture one of this launcher's own pages, at a stated size.

    python tools/appshot.py --page home --size 1280x720 --out shot.png
    python tools/appshot.py --exe dist/PalantirMC.exe --page home --out shot.png
    python tools/appshot.py --method shot --page home --out shot.png

Two ways to take the picture, and the default is the one whose pixels a *page*
gate can read:

* ``--method window`` (the default) launches the app at the stated size, waits
  for its window, pins the client area, and grabs the window as the compositor
  has it -- `PrintWindow` first, because that needs no foreground and no readable
  screen, then the screen as a fallback. Page and scrollbar are both there,
  because a person is looking at them.
* ``--method shot`` launches the app with its own ``--shot PATH`` flag: iced
  renders the frame it drew into a file and the window closes. Nothing is raised,
  the pointer never moves and no compositor is involved, which is exactly why it
  was the default until the pane became a scroll region.

Why the default moved, measured rather than assumed, on `757bbcd` -- the same
commit that built `dist/msvc/PalantirMC.exe`:

* a live window on `/browse/modpack` at a pinned 1280x720 client has **12,236**
  distinct colours in the page pane (x 64..1003, y 70..720): the tabs, the search
  field, the controls row, and the cards with their icons;
* the *same* build's ``--shot`` frame has **150** there, and every one of them
  lies in a strip x 970..1002 -- the scrollbar, and nothing the scrollbar scrolls;
* replacing the page's `scrollable` with a plain `container` (a scratch build,
  never committed) puts the page back into the ``--shot`` frame: 2,863 distinct
  colours in the pane. So the loss tracks the scroll region's clip, not the page
  and not the theme.

iced's `Scrollable` draws content that overflows it through `Renderer::with_layer`,
which ends in a `Primitive::Clip`; the offscreen render pass -- the one
`iced_wgpu` 0.12.1's `window::screenshot` builds for ``--shot`` -- drops that
layer's content, while the scrollbar drawn outside the layer survives. The same
build's ``kick-print`` capture, taken with the same command shape, has 11,954
there. So this is the capture *path*, not the page: the window itself is fine, and
what ``--shot`` cannot see is any page's scroll region.

The consequence for this tree is not cosmetic: `tools/page_gate.py` judges a page
from these captures, and a capture with no page in it makes that gate blind rather
than wrong, so a page port checked against it could pass on chrome alone. It did:
``--method shot`` on `757bbcd` fails that gate's `clusters` assertion with
`0 bands`.

What the two paths share is that nothing here clicks or moves the pointer. What
``--method window`` costs is stated plainly: the window is up for the settle plus
the grab, and the *fallback* -- only the fallback -- raises it, which is why
`PrintWindow` is tried first. Everything else this tool keeps: the launcher runs in
a throwaway directory holding `portable.dat`, so a capture reads its own empty data
root instead of yours, and the throwaway is removed on the way out -- with a debug
build it holds a copy of a 400 MB executable, so leaving it behind is a disk leak
in a tool run once per page.

What it prints is what the gates read: the file it wrote, its size, and which path
produced it. A failure to produce one is reported with the launcher's exit code so
a caller can tell "the app died" from "the app never took the picture".
"""
from __future__ import annotations

import argparse
import shutil
import subprocess
import sys
import tempfile
import time
from pathlib import Path

# `winshot` carries the Win32 attach/raise/grab helpers, and this tool is the
# walk's capture path with a sandbox around it rather than a second implementation
# of the same four calls. Imported the way `refwalk.py` imports it.
sys.path.insert(0, str(Path(__file__).resolve().parent))
import winshot as W  # noqa: E402  (the walk's own capture helpers)

# How long the launcher gets to appear, settle and write. The app's own settle
# timer is 3s (`app::SHOT_SETTLE`); the rest is startup, which on a debug build
# with a cold cache is a couple of seconds on its own.
TIMEOUT = 90.0
# How long a window capture waits after the window appears. The app's in-process
# settle is 3s and the page's own request rides on top of it -- a search over the
# network on `/browse/*` -- so this is the in-process number plus room for one
# round trip, which is what the page gates compare against.
SETTLE = 6.0


def sandbox(exe: Path) -> tuple[Path, Path]:
    """A throwaway directory holding `portable.dat` and a copy of the launcher.

    The marker is read *next to the executable* (`palantir_core::paths::
    portable_dir`), so the copy is the point: a run started from the sandbox
    directory but executing the real binary would find the reader's own data
    root and capture their instances.
    """
    directory = Path(tempfile.mkdtemp(prefix="appshot-"))
    (directory / "portable.dat").touch()
    copied = directory / exe.name
    shutil.copy2(exe, copied)
    return directory, copied


def sizes(size: str) -> tuple[int, int]:
    width, height = size.lower().split("x")
    return int(width), int(height)


def check(image_size: tuple[int, int], want: tuple[int, int], size: str) -> bool:
    if image_size == want:
        return True
    print(
        f"the capture is {image_size[0]}x{image_size[1]}, not the {size} asked for -- "
        f"every number a gate measures off it would be wrong by a scale factor",
        file=sys.stderr,
    )
    return False


def capture_shot(exe: Path, out: Path, size: str, page: str | None, keep: bool) -> int:
    """iced's own frame, written by the launcher (`--shot`)."""
    directory, copied = sandbox(exe)
    command = [str(copied), "--shot", str(out), "--size", size]
    if page:
        command += ["--page", page]

    started = time.time()
    proc = subprocess.Popen(command, cwd=directory)
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
        if not keep:
            shutil.rmtree(directory, ignore_errors=True)

    code = proc.returncode
    if not out.exists() or out.stat().st_size == 0:
        print(
            f"no capture: the launcher exited {code} without writing {out}",
            file=sys.stderr,
        )
        return 1

    from PIL import Image  # imported late so a missing Pillow fails after the run

    size_of = Image.open(out).size
    print(
        f"{out}: {size_of[0]}x{size_of[1]} via iced's own frame, written in "
        f"{time.time() - started:.1f}s (launcher exit {code})"
    )
    if not check(size_of, sizes(size), size):
        return 1
    if code not in (0, None):
        print(f"warning: the launcher exited {code}", file=sys.stderr)
    return 0


def capture_window(
    exe: Path, out: Path, size: str, page: str | None, settle: float, keep: bool
) -> int:
    """The window as the compositor has it -- the frame a person sees."""
    want = sizes(size)
    directory, copied = sandbox(exe)
    # No `--shot`: this run is meant to be an ordinary window, so iced presents it
    # to the compositor and the screen has something to grab.
    command = [str(copied), "--size", size]
    if page:
        command += ["--page", page]

    proc = subprocess.Popen(command, cwd=directory)
    hwnd = None
    try:
        # Wait for the *real* window rather than the first thing the process puts
        # on screen: a launcher opens small helper windows -- an IME host, a
        # hidden 6x6 -- before its own, and the largest visible window of the
        # process is only the right answer once it exists.
        for _ in range(160):
            if proc.poll() is not None:
                break
            candidate = W.find_window(pid=proc.pid)
            if candidate:
                left, top, right, bottom = W.window_rect(candidate)
                if (
                    right - left >= W.MIN_WINDOW_WIDTH
                    and bottom - top >= W.MIN_WINDOW_HEIGHT
                ):
                    hwnd = candidate
                    break
            time.sleep(0.25)
        if hwnd is None:
            print("no capture: the launcher never put a window up", file=sys.stderr)
            return 2

        got_w, got_h = W.resize_client(hwnd, *want)
        if (got_w, got_h) != want:
            print(
                f"WARNING: asked for a {want[0]}x{want[1]} client, the window "
                f"reports {got_w}x{got_h}",
                file=sys.stderr,
            )
        else:
            print(f"client pinned to {got_w}x{got_h}")

        time.sleep(settle)
        try:
            # `auto`: `PrintWindow`, then the screen. The first needs neither the
            # foreground nor a readable screen, and a capture session that raises
            # a window is a capture session somebody has to look at.
            image, method = W.grab(hwnd, "auto")
        except OSError as error:
            # `BitBlt` fails outright on a session with no display to read -- a
            # locked workstation, a machine that has gone to sleep. That is the
            # environment rather than the capture, so it is reported as one.
            print(f"capture failed: the screen could not be read ({error})", file=sys.stderr)
            return 3
        if image is None:
            print("capture failed", file=sys.stderr)
            return 3

        out.parent.mkdir(parents=True, exist_ok=True)
        if out.exists():
            out.unlink()
        image.save(out)
        ox, oy, client_w, client_h = W.client_origin(hwnd)
        print(
            f"{out}: {image.width}x{image.height} via {method} (client "
            f"{client_w}x{client_h} at screen {ox},{oy})"
        )
        return 0 if check(image.size, want, size) else 1
    finally:
        if proc.poll() is None:
            proc.terminate()
            try:
                proc.wait(timeout=10)
            except subprocess.TimeoutExpired:
                proc.kill()
                proc.wait()
        if not keep:
            shutil.rmtree(directory, ignore_errors=True)


def main() -> int:
    parser = argparse.ArgumentParser(
        description=__doc__,
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )
    parser.add_argument(
        "--exe",
        default="target/debug/PalantirMC.exe",
        help="the launcher to capture (default: the debug build)",
    )
    parser.add_argument(
        "--page",
        default=None,
        help="the page to open on, passed straight through as --page",
    )
    parser.add_argument(
        "--size",
        default="1280x720",
        help="client size to capture at (default 1280x720, the size "
        "the reference is measured at)",
    )
    parser.add_argument(
        "--method",
        choices=("window", "shot"),
        default="window",
        help="window: the composited window, the only path that contains a page "
        "(default); shot: iced's own frame, which does not draw a scroll "
        "region's content -- see this file's docstring",
    )
    parser.add_argument(
        "--settle",
        type=float,
        default=SETTLE,
        help=f"seconds a window capture waits after the window appears "
        f"(default {SETTLE}, ignored by --method shot)",
    )
    parser.add_argument("--out", required=True, help="where to write the PNG")
    parser.add_argument(
        "--keep",
        action="store_true",
        help="leave the throwaway directory behind, for a run that "
        "has to be diagnosed from its own files",
    )
    args = parser.parse_args()

    exe = Path(args.exe).resolve()
    if not exe.exists():
        print(f"no such executable: {exe}", file=sys.stderr)
        return 2
    out = Path(args.out).resolve()

    if args.method == "shot":
        return capture_shot(exe, out, args.size, args.page, args.keep)
    # `GetWindowRect` answers in scaled units without this while `ImageGrab`
    # answers in real ones, which crops the fallback's capture to the wrong
    # rectangle.
    W.make_dpi_aware()
    return capture_window(exe, out, args.size, args.page, args.settle, args.keep)


if __name__ == "__main__":
    raise SystemExit(main())
