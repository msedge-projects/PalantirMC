#!/usr/bin/env python3
"""Drive the reference client's window and capture what it draws.

Why this exists, and why it is not `winshot.py`: `winshot.py` captures a window
without touching the desk, which is right for this launcher and wrong for the
reference. The reference is a WebView2 window, and `NEXT_STEPS.md` §14 already
measured that injected mouse and key messages reach neither a Chromium window nor
our own iced shell, so every click here is the *real* pointer: the window is
raised, the cursor is moved to a client coordinate inside it, and the button is
pressed. Nothing else on the desktop is clicked, because every click is preceded
by an activation and the coordinates are client coordinates of that one window.

The captures are cropped to the *client* area with the inset measured from the
window rather than assumed, which is the mistake `panel_gate.py` was revised
twice for: a 1280x720 window is not a 1280x720 picture of a 1280x720 layout when
it carries a frame, and a click at a coordinate read off an uncropped capture
lands on the wrong control.

    python tools/refwalk.py --attach "Modrinth App" --size 1280x720 \\
        --script .scratch/ref-session.txt --out-dir .scratch

Script lines, one action per line, `#` comments welcome:

    show                  raise the window, and show it if it was created hidden
    resize W H            pin the window so two clients' captures compare 1:1
    activate              bring it to the foreground before a real click
    click X Y             real pointer: move, press, release (client coords)
    hover X Y             real pointer move only, for a tooltip
    key VK                real key press by virtual-key code (0x1B is Escape)
    type TEXT             real keystrokes, for a search box
    wait S                let the UI settle before the next action
    shot NAME             capture to <out-dir>/ref-<NAME>.png, and OCR it
    ocr NAME              re-run OCR on an existing capture
    sample NAME X0 Y0 X1 Y1   print a region's structure and labels
    note TEXT             a line in the log, so a session explains itself

Every action is appended to `<out-dir>/refwalk.log` with its result, because a
walk whose steps are not recorded is a walk nobody can repeat or audit.
"""
from __future__ import annotations

import argparse
import ctypes
import ctypes.wintypes as w
import os
import subprocess
import sys
import time
from pathlib import Path

import numpy as np
from PIL import Image

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import winshot as W  # noqa: E402  (attach, grab, activate, pointer)

import refsample as RS  # noqa: E402

user32 = W.user32

VK_ESCAPE = 0x1B
VK_RETURN = 0x0D
VK_TAB = 0x09
KEYEVENTF_KEYUP = 0x0002
INPUT_KEYBOARD = 1


def key_tap(vk: int, hold: float = 0.05) -> None:
    """Press and release a key for real.

    `keybd_event` rather than a posted message, for the reason in §14: Chromium
    ignores the message, and a text field that never receives the keystroke looks
    exactly like a search box that found nothing.
    """
    user32.keybd_event(vk, 0, 0, None)
    time.sleep(hold)
    user32.keybd_event(vk, 0, KEYEVENTF_KEYUP, None)
    time.sleep(0.05)


def type_text(text: str) -> None:
    """Type into whatever has focus, one character at a time.

    `VkKeyScanW` maps a character to its virtual key plus the modifier state it
    needs, which is what keeps capitals and punctuation from arriving as their
    unshifted keys.
    """
    for ch in text:
        packed = user32.VkKeyScanW(ord(ch))
        if packed == -1:
            continue
        vk = packed & 0xFF
        shift = bool(packed & 0x0100)
        if shift:
            user32.keybd_event(0x10, 0, 0, None)
        key_tap(vk, 0.02)
        if shift:
            user32.keybd_event(0x10, 0, KEYEVENTF_KEYUP, None)
        time.sleep(0.02)


class RefWindow:
    """The reference client's window, and the client rectangle inside it."""

    def __init__(self, title: str, out_dir: Path, log: Path) -> None:
        self.hwnd = W.find_window(title=title)
        if not self.hwnd:
            raise SystemExit(f"no visible window whose title contains {title!r}")
        self.out_dir = out_dir
        self.log_path = log
        self.inset = (0, 0)
        self.client = (0, 0)
        self.note(f"attached hwnd={self.hwnd} rect={W.window_rect(self.hwnd)}")

    # -- reporting ---------------------------------------------------------

    def note(self, text: str) -> None:
        line = f"[{time.strftime('%H:%M:%S')}] {text}"
        print(line, flush=True)
        with open(self.log_path, "a", encoding="utf-8") as handle:
            handle.write(line + "\n")

    # -- window state ------------------------------------------------------

    def show(self) -> None:
        """Raise it, show it if it was created hidden, and restore it if minimised.

        Both of the first two are measured rather than assumed, and each cost a
        wrong capture:

        * Launched from a background shell this app creates its Tauri window with
          `WS_VISIBLE` unset and leaves it that way -- `IsWindowVisible` false,
          `PrintWindow` refusing, no error anywhere.
        * A restored-from-minimised window reports the rect it will have when
          restored, so a resize during that state changes the *restore* rectangle
          while `GetClientRect` keeps answering with the last real one. The
          capture then comes back at the old size with the new size around it,
          and every coordinate in it is wrong -- which is exactly what happened
          on the first session (a 1288x720 window rect with a 1366x767 client).
        """
        if user32.IsIconic(self.hwnd):
            user32.ShowWindow(self.hwnd, W.SW_RESTORE)
            time.sleep(0.6)
        if not user32.IsWindowVisible(self.hwnd):
            user32.ShowWindow(self.hwnd, W.SW_SHOW)
            time.sleep(0.5)
        W.activate(self.hwnd)
        self.note(f"shown; visible={bool(user32.IsWindowVisible(self.hwnd))} "
                  f"iconic={bool(user32.IsIconic(self.hwnd))} "
                  f"maximized={bool(user32.IsZoomed(self.hwnd))} "
                  f"foreground={user32.GetForegroundWindow() == self.hwnd}")

    def resize(self, width: int, height: int, want_client: tuple[int, int] | None = None) -> None:
        """Pin the window's size, with no activation and no z-order change.

        Restores first, because `SetWindowPos` on a maximised window resizes the
        restored rectangle while the window stays maximised -- so the layout keeps
        the maximised width and the capture disagrees with the size that was asked
        for. `want_client`, when given, is then checked against what the app
        actually reports, and a mismatch is reported rather than captured over.
        """
        if user32.IsIconic(self.hwnd) or user32.IsZoomed(self.hwnd):
            user32.ShowWindow(self.hwnd, W.SW_RESTORE)
            time.sleep(0.8)
        width, height = int(width), int(height)
        for _ in range(6):
            user32.SetWindowPos(self.hwnd, None, 0, 0, width, height,
                                W.SWP_NOZORDER | W.SWP_NOACTIVATE | W.SWP_NOMOVE)
            time.sleep(0.5)
            self.measure()
            if user32.IsIconic(self.hwnd) or user32.IsZoomed(self.hwnd):
                user32.ShowWindow(self.hwnd, W.SW_RESTORE)
                time.sleep(0.5)
                continue
            if want_client is None or self.client == tuple(want_client):
                break
            # A frame's borders are not the same on all four edges (this window's
            # are 8px left/top and 0--1px right/bottom), so the window size that
            # produces a given client is solved for rather than calculated.
            width += want_client[0] - self.client[0]
            height += want_client[1] - self.client[1]
        if want_client is not None and self.client != tuple(want_client):
            self.note(f"WARNING: asked for a {want_client} client and the window "
                      f"reports {self.client} (inset {self.inset}, window {width}x{height})")
        # The webview reflows after a resize; a capture taken in the same frame
        # catches the old layout with the new size around it.
        time.sleep(1.2)

    def state(self) -> None:
        """What the window believes about itself, before anything is read off it."""
        self.measure()
        self.note(f"state visible={bool(user32.IsWindowVisible(self.hwnd))} "
                  f"iconic={bool(user32.IsIconic(self.hwnd))} "
                  f"maximized={bool(user32.IsZoomed(self.hwnd))} "
                  f"foreground={user32.GetForegroundWindow() == self.hwnd}")

    def measure(self) -> None:
        """Where the client area sits inside the window, in real pixels."""
        r = w.RECT()
        user32.GetClientRect(self.hwnd, ctypes.byref(r))
        point = w.POINT(0, 0)
        user32.ClientToScreen(self.hwnd, ctypes.byref(point))
        left, top, _, _ = W.window_rect(self.hwnd)
        self.inset = (point.x - left, point.y - top)
        self.client = (r.right - r.left, r.bottom - r.top)
        self.note(f"window={W.window_rect(self.hwnd)} inset={self.inset} client={self.client}")

    # -- actions -----------------------------------------------------------

    def click(self, x: float, y: float, settle: float = 1.4) -> None:
        self.show()
        W.click_client(self.hwnd, x, y)
        time.sleep(settle)
        self.note(f"click {int(x)},{int(y)}")

    def hover(self, x: float, y: float, settle: float = 1.6) -> None:
        self.show()
        ox, oy, _, _ = W.client_origin(self.hwnd)
        user32.SetCursorPos(int(ox + x), int(oy + y))
        time.sleep(settle)
        self.note(f"hover {int(x)},{int(y)}")

    def capture(self, name: str) -> Path:
        """A client-area capture, plus OCR of it."""
        image, how = W.grab(self.hwnd, "print")
        if image is None:
            self.note(f"capture {name}: PrintWindow refused, using the screen")
            image, how = W.grab(self.hwnd, "screen")
        if image is None:
            raise SystemExit(f"capture {name} failed")
        dx, dy = self.inset
        cw, ch = self.client
        image = image.crop((dx, dy, dx + cw, dy + ch))
        path = self.out_dir / f"ref-{name}.png"
        image.save(path)
        self.note(f"shot {path.name} {image.size[0]}x{image.size[1]} via {how}")
        return path


def run_script(window: RefWindow, lines: list[str]) -> int:
    for number, raw in enumerate(lines, 1):
        line = raw.split("#", 1)[0].strip()
        if not line:
            continue
        parts = line.split()
        verb = parts[0].lower()
        try:
            if verb == "show":
                window.show()
            elif verb == "state":
                window.state()
            elif verb == "resize" and len(parts) in (3, 5):
                want = None
                if len(parts) == 5:
                    want = (int(parts[3]), int(parts[4]))
                window.resize(int(parts[1]), int(parts[2]), want)
            elif verb == "activate":
                window.show()
            elif verb == "click" and len(parts) == 3:
                window.click(float(parts[1]), float(parts[2]),
                             float(parts[3]) if len(parts) > 3 else 1.4)
            elif verb == "hover" and len(parts) == 3:
                window.hover(float(parts[1]), float(parts[2]),
                             float(parts[3]) if len(parts) > 3 else 1.6)
            elif verb == "key" and len(parts) == 2:
                key_tap(int(parts[1], 0))
                window.note(f"key {parts[1]}")
                time.sleep(0.8)
            elif verb == "type" and len(parts) >= 2:
                text = " ".join(parts[1:])
                type_text(text)
                window.note(f"type {text!r}")
                time.sleep(1.2)
            elif verb == "wait" and len(parts) == 2:
                time.sleep(float(parts[1]))
            elif verb == "shot" and len(parts) == 2:
                path = window.capture(parts[1])
                RS.ocr(path, refresh=True)
            elif verb == "ocr" and len(parts) == 2:
                path = window.out_dir / f"ref-{parts[1]}.png"
                for ln in RS.ocr(path, refresh=True):
                    window.note(f"  ocr {ln['x']:>4},{ln['y']:>4} {ln['text']}")
            elif verb == "sample" and len(parts) == 6:
                path = window.out_dir / f"ref-{parts[1]}.png"
                r = RS.region(path, [int(v) for v in parts[2:6]])
                window.note(f"  region {r['box']} palette=" +
                            ", ".join(f"{c['hex']}({c['coverage'] * 100:.0f}%)"
                                      for c in r["palette"][:6]))
                for b in r["blocks"]:
                    window.note(f"    block y {b['y0']}..{b['y1']} h={b['h']} "
                                f"x {b['x0']}..{b['x1']}")
                for ln in r["labels"]:
                    window.note(f"    label {ln['x']:>4},{ln['y']:>4} {ln['text']}")
            elif verb == "note":
                window.note(" ".join(parts[1:]))
            else:
                window.note(f"line {number}: cannot read {line!r}")
                return 1
        except Exception as exc:  # noqa: BLE001 - a session should report, not die
            window.note(f"line {number} ({line!r}) failed: {exc}")
            return 1
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__,
                                     formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--attach", default="Modrinth App",
                        help="window title substring to attach to")
    parser.add_argument("--size", default=None,
                        help="pin the window to SWxSH before the script runs")
    parser.add_argument("--script", help="file of actions (see the module docstring)")
    parser.add_argument("--out-dir", default=".scratch")
    args = parser.parse_args()

    W.make_dpi_aware()
    out_dir = Path(args.out_dir)
    out_dir.mkdir(parents=True, exist_ok=True)
    window = RefWindow(args.attach, out_dir, out_dir / "refwalk.log")
    window.show()
    if args.size:
        width, height = (int(v) for v in args.size.lower().split("x"))
        # The inset is known by now, so the client size this should produce is a
        # checkable expectation rather than a hope.
        window.resize(width, height,
                      (width - 2 * window.inset[0], height - 2 * window.inset[1]))
    window.state()
    if not args.script:
        return 0
    return run_script(window, Path(args.script).read_text(encoding="utf-8").splitlines())


if __name__ == "__main__":
    raise SystemExit(main())
