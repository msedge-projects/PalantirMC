#!/usr/bin/env python3
"""Capture one screen state of a GUI app on a headless X display, for 1:1 parity checks.

Written for the `sift-launcher` VM: there is no real display, so both the Modrinth
app and Sift Launcher are drawn on an `Xvfb` screen and read back with ImageMagick.
The report line it writes is the **same format** `tools/parity/compose.py` parses, so a
capture taken here and one taken anywhere else can be compared by the same tool.

Why a separate script from `capture.py`: that one drives Win32 through PowerShell. This
one is X11 and only needs `xdotool` and `import`, both already on the VM. Two platforms,
one report format.

Usage
-----
    python3 tools/parity/capture_x11.py --app Modrinth --page discover --out out
    python3 tools/parity/capture_x11.py --app "Sift Launcher" --page home --out out

Exit codes: 0 ok, 1 usage or pages file, 2 no window matched, 3 capture failed.
"""

from __future__ import annotations

import argparse
import json
import subprocess
import sys
from pathlib import Path

#: The window size every capture is pinned to. Two apps at different sizes are not a
#: comparison, so this is a constant rather than a flag: `--size` exists only to
#: override it deliberately, e.g. to check a page at its minimum window.
DEFAULT_SIZE = (1440, 900)

#: How long to let a window settle after it is raised or resized, in milliseconds.
#: Long enough for a webview page to finish its entrance transition (the reference's
#: longest is 400 ms) and short enough not to make a capture batch tedious.
DEFAULT_SETTLE_MS = 2000


def run(args: list[str], timeout: float = 60) -> subprocess.CompletedProcess:
    """Run a command, capturing its text, without ever raising on a non-zero status."""
    return subprocess.run(
        args, capture_output=True, text=True, timeout=timeout, check=False
    )


def find_window(title: str, display: str) -> str | None:
    """The id of the first window whose title contains `title`, or None."""
    result = run(["xdotool", "search", "--name", title], timeout=20)
    for line in result.stdout.splitlines():
        window = line.strip()
        if window:
            return window
    return None


def open_titles(display: str) -> list[str]:
    """Every window title currently on the display, for the error message."""
    result = run(["xdotool", "search", "--name", "."], timeout=20)
    titles = []
    for line in result.stdout.splitlines():
        window = line.strip()
        if not window:
            continue
        name = run(["xdotool", "getwindowname", window], timeout=10)
        if name.stdout.strip():
            titles.append(name.stdout.strip())
    return titles


def rect_of(window: str) -> tuple[int, int, int, int]:
    """The window's (x, y, width, height), or zeros when it cannot be read."""
    result = run(["xdotool", "getwindowgeometry", "--shell", window], timeout=10)
    values: dict[str, int] = {}
    for line in result.stdout.splitlines():
        if "=" in line:
            key, _, value = line.partition("=")
            try:
                values[key.strip()] = int(value.strip())
            except ValueError:
                continue
    return (
        values.get("X", 0),
        values.get("Y", 0),
        values.get("WIDTH", 0),
        values.get("HEIGHT", 0),
    )


def apply_step(step: dict) -> list[str]:
    """The xdotool commands for one step of a page's script."""
    if "move" in step:
        x, y = step["move"]
        return [["xdotool", "mousemove", str(x), str(y)]]
    if "click" in step:
        x, y = step["click"]
        return [["xdotool", "mousemove", str(x), str(y)], ["xdotool", "click", "1"]]
    if "wheel" in step:
        x, y, notches = step["wheel"]
        # xdotool's wheel buttons are 4 (up) and 5 (down); one click is one notch,
        # which is exactly what a physical wheel sends.
        button = "4" if notches > 0 else "5"
        commands = [["xdotool", "mousemove", str(x), str(y)]]
        for _ in range(abs(int(notches))):
            commands.append(["xdotool", "click", button])
        return commands
    if "key" in step:
        return [["xdotool", "key", step["key"]]]
    if "wait" in step:
        return [["sleep", f"{int(step['wait']) / 1000:.3f}"]]
    raise ValueError(f"unknown step {step!r}")


def sample_points(png: Path, inset: int = 8) -> list[tuple[int, int, int]]:
    """The RGB of five points, in the order `capture.py` uses: centre, then the four
    inset corners (TL, TR, BL, BR).

    Five rather than four because the centre is the one point that samples the page's
    own background in every page, so it catches a whole-theme drift that four corners
    of a window could all miss. Sampled from the saved file rather than from the
    display so the report describes the artifact that will be compared, not a moment
    that has already passed.
    """
    geometry = run(["identify", "-format", "%w %h", str(png)], timeout=20)
    try:
        width, height = (int(part) for part in geometry.stdout.split())
    except ValueError:
        return []
    points = [
        (width // 2, height // 2),
        (inset, inset),
        (width - inset, inset),
        (inset, height - inset),
        (width - inset, height - inset),
    ]
    samples = []
    for x, y in points:
        probe = run(
            ["convert", str(png), "-format", f"%[pixel:p{{{x},{y}}}]", "info:"],
            timeout=20,
        )
        text = probe.stdout.strip()
        # `srgb(15,15,15)` in ImageMagick 7, `srgb(15,15,15)` or `#0F0F0F` in 6.
        numbers = [
            int(n)
            for n in text.replace("srgb(", "").replace(")", "").split(",")[:3]
            if n.strip().isdigit()
        ]
        if len(numbers) == 3:
            samples.append((numbers[0], numbers[1], numbers[2]))
    return samples


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--app", required=True, help="substring of the window title")
    parser.add_argument("--slug", default=None, help="file suffix (default: from --app)")
    parser.add_argument("--out", required=True, type=Path)
    parser.add_argument("--page", required=True)
    parser.add_argument("--state", default="idle")
    parser.add_argument("--display", default=":99")
    parser.add_argument("--size", default=f"{DEFAULT_SIZE[0]}x{DEFAULT_SIZE[1]}",
                        help="window size to pin, or 'none'")
    parser.add_argument("--settle-ms", type=int, default=DEFAULT_SETTLE_MS)
    parser.add_argument("--pages", type=Path,
                        default=Path(__file__).with_name("pages.json"))
    args = parser.parse_args()

    try:
        pages = json.loads(args.pages.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        print(f"capture_x11: cannot read {args.pages}: {error}", file=sys.stderr)
        return 1
    if args.page not in pages:
        known = ", ".join(sorted(k for k in pages if not k.startswith("_")))
        print(f"capture_x11: unknown page {args.page!r}; known: {known}", file=sys.stderr)
        return 1

    window = find_window(args.app, args.display)
    if window is None:
        print(
            f"capture_x11: no window matching {args.app!r}; open windows: "
            f"{open_titles(args.display) or 'none'}",
            file=sys.stderr,
        )
        return 2

    run(["xdotool", "windowactivate", "--sync", window], timeout=30)
    if args.size != "none":
        run(["xdotool", "windowsize", window, *args.size.split("x")], timeout=30)
    run(["xdotool", "windowmove", window, "0", "0"], timeout=30)
    run(["sleep", f"{args.settle_ms / 1000:.3f}"], timeout=60)

    for step in pages[args.page].get("steps", []):
        if isinstance(step, str):
            # A named constant from the pages file's `constants` table.
            step = pages["constants"][step]
        for command in apply_step(step):
            run(command, timeout=30)
    run(["sleep", f"{args.settle_ms / 1000:.3f}"], timeout=60)

    args.out.mkdir(parents=True, exist_ok=True)
    slug = args.slug or ("modrinth" if "modrinth" in args.app.lower() else "sift")
    png = args.out / f"{args.page}-{args.state}-{slug}.png"
    if png.exists():
        png.unlink()
    captured = run(["import", "-display", args.display, "-window", window, str(png)],
                   timeout=60)
    if captured.returncode != 0 or not png.exists():
        print(f"capture_x11: import failed: {captured.stderr.strip()}", file=sys.stderr)
        return 3

    x, y, width, height = rect_of(window)
    corners = ";".join(f"{r},{g},{b}" for r, g, b in sample_points(png))
    line = (
        f"app={slug} page={args.page} state={args.state} "
        f"rect={x},{y},{width},{height} corners={corners}\n"
    )
    with (args.out / "report.txt").open("a", encoding="utf-8") as report:
        report.write(line)
    print(f"wrote {png} ({width}x{height}) and one report line")
    return 0


if __name__ == "__main__":
    sys.exit(main())
