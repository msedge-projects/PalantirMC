#!/usr/bin/env python3
"""Capture a running (or to-be-launched) window to a PNG.

Why this exists next to `launch_check.py`: comparing this shell against the
client it is modelled on is a visual judgement, and a visual judgement made from
memory is a guess. This turns the window into a file so the comparison is
between two images rather than between two recollections.

Two capture paths, tried in that order:

* ``PrintWindow`` with ``PW_RENDERFULLCONTENT`` asks the window to draw itself
  into a bitmap. It does not need the window to be visible, focused or on top,
  so on a machine somebody else is using it costs nothing -- no cursor, no
  focus change, no window raised over their work.
* ``ImageGrab`` of the window's rectangle, which needs the window actually
  visible. Used only when the first path comes back blank, and it will raise the
  window, so it is announced on stdout when it happens.

The first path is why the option exists: a Chromium-based window (the reference
client is Tauri) often refuses to draw itself off-screen, and this shell does not.

    python tools/winshot.py --launch path/to/App.exe --settle 6 --out shot.png
    python tools/winshot.py --title "Modrinth App" --out shot.png
    python tools/winshot.py --launch ... --click 92,180 --click 92,240 --out x.png

``--click`` takes client-area coordinates and is applied in order, each followed
by ``--click-settle``, so a screenshot series across pages is one invocation.
"""

import argparse
import ctypes
import ctypes.wintypes as w
import os
import subprocess
import sys
import tempfile
import time

import numpy as np
from PIL import Image, ImageGrab

user32 = ctypes.WinDLL("user32", use_last_error=True)
gdi32 = ctypes.WinDLL("gdi32", use_last_error=True)

user32.GetWindowRect.argtypes = [w.HWND, ctypes.POINTER(w.RECT)]
user32.GetClientRect.argtypes = [w.HWND, ctypes.POINTER(w.RECT)]
user32.PrintWindow.argtypes = [w.HWND, w.HDC, w.UINT]
user32.ShowWindow.argtypes = [w.HWND, ctypes.c_int]
user32.SetForegroundWindow.argtypes = [w.HWND]
user32.GetForegroundWindow.restype = w.HWND
user32.SetCursorPos.argtypes = [ctypes.c_int, ctypes.c_int]
user32.ClientToScreen.argtypes = [w.HWND, ctypes.POINTER(w.POINT)]
user32.IsWindow.argtypes = [w.HWND]
user32.IsWindowVisible.argtypes = [w.HWND]

PW_RENDERFULLCONTENT = 0x00000002
SW_RESTORE = 9
SW_SHOW = 5

ENUMPROC = ctypes.WINFUNCTYPE(w.BOOL, w.HWND, w.LPARAM)


def make_dpi_aware():
    """Report real pixels.

    Without this, `GetWindowRect` answers in scaled units on a scaled display
    while `ImageGrab` answers in real ones, and the crop silently lands on the
    wrong part of the screen.
    """
    try:
        ctypes.WinDLL("shcore").SetProcessDpiAwareness(2)
    except Exception:
        try:
            user32.SetProcessDPIAware()
        except Exception:
            pass


def window_rect(hwnd):
    r = w.RECT()
    user32.GetWindowRect(hwnd, ctypes.byref(r))
    return (r.left, r.top, r.right, r.bottom)


def client_origin(hwnd):
    """Where the client area's (0, 0) is, in screen pixels."""
    r = w.RECT()
    user32.GetClientRect(hwnd, ctypes.byref(r))
    point = w.POINT(0, 0)
    user32.ClientToScreen(hwnd, ctypes.byref(point))
    return (point.x, point.y, r.right - r.left, r.bottom - r.top)


def find_window(pid=None, title=None):
    """The largest visible top-level window owned by `pid`, or matching `title`."""
    found = []

    @ENUMPROC
    def visit(hwnd, _):
        if not user32.IsWindowVisible(hwnd) or user32.GetWindow(hwnd, 4):
            return True
        if pid is not None:
            owner = w.DWORD()
            user32.GetWindowThreadProcessId(hwnd, ctypes.byref(owner))
            if owner.value != pid:
                return True
        if title is not None:
            length = user32.GetWindowTextLengthW(hwnd)
            buffer = ctypes.create_unicode_buffer(length + 1)
            user32.GetWindowTextW(hwnd, buffer, length + 1)
            if title.lower() not in buffer.value.lower():
                return True
        found.append(hwnd)
        return True

    user32.EnumWindows(visit, 0)
    if not found:
        return None
    found.sort(key=lambda h: (window_rect(h)[2] - window_rect(h)[0])
               * (window_rect(h)[3] - window_rect(h)[1]), reverse=True)
    return found[0]


def pid_of(name):
    out = subprocess.run(["tasklist", "/FI", f"IMAGENAME eq {name}", "/FO", "CSV", "/NH"],
                         capture_output=True, text=True).stdout
    for line in out.splitlines():
        parts = [p.strip('"') for p in line.split('","')]
        if len(parts) >= 2 and parts[0].lower() == name.lower():
            return int(parts[1])
    return None


def activate(hwnd):
    """Raise a window reliably. Windows refuses a foreground change from a
    process that is not already in the foreground unless the caller has just
    received input, so tap Alt first -- that counts."""
    if user32.IsIconic(hwnd):
        user32.ShowWindow(hwnd, SW_RESTORE)
    for _ in range(25):
        if user32.GetForegroundWindow() == hwnd:
            return True
        user32.keybd_event(0x12, 0, 0, None)
        user32.keybd_event(0x12, 0, 2, None)
        user32.SetForegroundWindow(hwnd)
        user32.ShowWindow(hwnd, SW_SHOW)
        time.sleep(0.25)
    return user32.GetForegroundWindow() == hwnd


def grab(hwnd, method="auto"):
    """A PIL image of the window. Returns (image, method_used)."""
    left, top, right, bottom = window_rect(hwnd)
    width, height = right - left, bottom - top

    if method in ("auto", "print"):
        screen_dc = user32.GetDC(0)
        mem_dc = gdi32.CreateCompatibleDC(screen_dc)
        bitmap = gdi32.CreateCompatibleBitmap(screen_dc, width, height)
        gdi32.SelectObject(mem_dc, bitmap)
        ok = user32.PrintWindow(hwnd, mem_dc, PW_RENDERFULLCONTENT)

        class BITMAPINFOHEADER(ctypes.Structure):
            _fields_ = [("biSize", w.DWORD), ("biWidth", ctypes.c_long),
                        ("biHeight", ctypes.c_long), ("biPlanes", w.WORD),
                        ("biBitCount", w.WORD), ("biCompression", w.DWORD),
                        ("biSizeImage", w.DWORD), ("biXPelsPerMeter", ctypes.c_long),
                        ("biYPelsPerMeter", ctypes.c_long), ("biClrUsed", w.DWORD),
                        ("biClrImportant", w.DWORD)]

        header = BITMAPINFOHEADER()
        header.biSize = ctypes.sizeof(BITMAPINFOHEADER)
        header.biWidth = width
        header.biHeight = -height  # top-down
        header.biPlanes = 1
        header.biBitCount = 32
        header.biCompression = 0  # BI_RGB
        buffer = ctypes.create_string_buffer(width * height * 4)
        gdi32.GetDIBits(mem_dc, bitmap, 0, height, buffer, ctypes.byref(header), 0)

        gdi32.DeleteObject(bitmap)
        gdi32.DeleteDC(mem_dc)
        user32.ReleaseDC(0, screen_dc)

        if ok:
            image = Image.frombuffer("RGBA", (width, height), buffer, "raw", "BGRA", 0, 1)
            pixels = np.asarray(image.convert("RGB"), dtype=np.uint8)
            # A request that was refused comes back uniformly black or uniformly
            # one colour, which is not a screenshot of anything.
            if pixels.std() > 2.0:
                return image.convert("RGB"), "print"
        if method == "print":
            return None, "print-failed"

    activate(hwnd)
    time.sleep(0.7)
    left, top, right, bottom = window_rect(hwnd)
    image = ImageGrab.grab(bbox=(left, top, right, bottom), all_screens=True)
    return image.convert("RGB"), "screen"


def click_client(hwnd, x, y):
    ox, oy, _, _ = client_origin(hwnd)
    user32.SetCursorPos(int(ox + x), int(oy + y))
    time.sleep(0.15)
    user32.mouse_event(0x0002, 0, 0, 0, 0)
    time.sleep(0.05)
    user32.mouse_event(0x0004, 0, 0, 0, 0)


def main():
    parser = argparse.ArgumentParser(description=__doc__,
                                     formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--launch", help="executable to start")
    parser.add_argument("--cwd", help="working directory for the launch")
    parser.add_argument("--portable", action="store_true",
                        help="launch in a throwaway directory holding portable.dat, "
                             "so a sandboxed app cannot touch real data")
    parser.add_argument("--title", help="attach to a window whose title contains this")
    parser.add_argument("--process", help="attach to a running process by image name")
    parser.add_argument("--settle", type=float, default=6.0,
                        help="seconds to wait after the window appears (default 6)")
    parser.add_argument("--click", action="append", default=[],
                        help="client-area X,Y to click before capturing; repeatable")
    parser.add_argument("--click-settle", type=float, default=1.2)
    parser.add_argument("--method", choices=("auto", "print", "screen"), default="auto")
    parser.add_argument("--out", required=True)
    parser.add_argument("--keep", action="store_true",
                        help="leave a launched process running")
    args = parser.parse_args()

    make_dpi_aware()

    proc = None
    sandbox = None
    if args.launch:
        cwd = args.cwd
        if args.portable:
            sandbox = tempfile.mkdtemp(prefix="winshot-")
            open(os.path.join(sandbox, "portable.dat"), "w").close()
            cwd = sandbox
        # Resolve before the cwd moves: `--portable` points the child at a
        # throwaway directory, so a relative executable path stops existing the
        # moment that happens.
        proc = subprocess.Popen([os.path.abspath(args.launch)], cwd=cwd)
        hwnd = None
        for _ in range(160):
            hwnd = find_window(pid=proc.pid)
            if hwnd:
                break
            time.sleep(0.25)
    else:
        name = args.process or (args.title and (args.title + ".exe"))
        pid = pid_of(name) if name else None
        hwnd = find_window(pid=pid) if pid else None
        if hwnd is None:
            hwnd = find_window(title=args.title)

    if hwnd is None:
        print("no window found", file=sys.stderr)
        return 2

    time.sleep(args.settle)

    for click in args.click:
        x, y = (float(v) for v in click.split(","))
        click_client(hwnd, x, y)
        time.sleep(args.click_settle)

    image, method = grab(hwnd, args.method)
    if image is None:
        print("capture failed", file=sys.stderr)
        return 3

    os.makedirs(os.path.dirname(os.path.abspath(args.out)), exist_ok=True)
    image.save(args.out)
    ox, oy, cw, ch = client_origin(hwnd)
    print(f"{args.out}: {image.width}x{image.height} via {method} "
          f"(client {cw}x{ch} at screen {ox},{oy})")

    if proc is not None and not args.keep:
        proc.terminate()
        try:
            proc.wait(timeout=10)
        except subprocess.TimeoutExpired:
            proc.kill()
    if sandbox:
        import shutil
        shutil.rmtree(sandbox, ignore_errors=True)
    return 0


if __name__ == "__main__":
    sys.exit(main())
