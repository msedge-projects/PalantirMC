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
    python tools/winshot.py --launch app.exe --script session.txt --settle 6

``--click`` takes client-area coordinates and is applied in order, each followed
by ``--click-settle``, so a screenshot series across pages is one invocation --
but it moves the real pointer. ``--script`` is the one to use on a machine
somebody is working on: it posts the messages the mouse would have sent, so it
stages a whole session of clicks and screenshots with no cursor, no focus change
and no window raised. Script lines are ``click X Y``, ``shot OUT.png`` and
``wait SECONDS``.
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
user32.PostMessageW.argtypes = [w.HWND, w.UINT, w.WPARAM, w.LPARAM]
user32.PostMessageW.restype = w.BOOL
user32.SendMessageW.argtypes = [w.HWND, w.UINT, w.WPARAM, w.LPARAM]
user32.SendMessageW.restype = w.LPARAM
user32.MapVirtualKeyW.argtypes = [w.UINT, w.UINT]
user32.MapVirtualKeyW.restype = w.UINT
user32.SetWindowPos.argtypes = [w.HWND, w.HWND, ctypes.c_int, ctypes.c_int,
                                ctypes.c_int, ctypes.c_int, w.UINT]
user32.SetWindowPos.restype = w.BOOL
user32.GetSystemMetrics.argtypes = [ctypes.c_int]
user32.GetSystemMetrics.restype = ctypes.c_int
user32.ClientToScreen.argtypes = [w.HWND, ctypes.POINTER(w.POINT)]
user32.IsWindow.argtypes = [w.HWND]
user32.IsWindowVisible.argtypes = [w.HWND]

PW_RENDERFULLCONTENT = 0x00000002
WM_MOUSEMOVE = 0x0200
WM_LBUTTONDOWN = 0x0201
WM_LBUTTONUP = 0x0202
MK_LBUTTON = 0x0001
WM_ACTIVATE = 0x0006
WM_SETFOCUS = 0x0007
WM_KEYDOWN = 0x0100
WM_KEYUP = 0x0101
WM_CHAR = 0x0102
MAPVK_VK_TO_VSC = 0
SWP_NOSIZE = 0x0001
SWP_NOZORDER = 0x0004
SWP_NOACTIVATE = 0x0010
SWP_NOMOVE = 0x0002
SM_XVIRTUALSCREEN = 76
SM_CXVIRTUALSCREEN = 78
SW_RESTORE = 9
SW_SHOW = 5

# Below this a "window" is a helper, an IME host or a tooltip rather than the
# app's own frame.
MIN_WINDOW_WIDTH = 400
MIN_WINDOW_HEIGHT = 300

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
    try:
        image = ImageGrab.grab(bbox=(left, top, right, bottom), all_screens=True)
    except OSError:
        # A grab can lose the race with the window being raised; one retry after
        # a longer pause is cheaper than a whole re-run.
        time.sleep(1.5)
        left, top, right, bottom = window_rect(hwnd)
        image = ImageGrab.grab(bbox=(left, top, right, bottom), all_screens=True)
    return image.convert("RGB"), "screen"


def click_client(hwnd, x, y):
    """Click by moving the real pointer. Steals the cursor from whoever is using
    the machine, so `post_click_client` is what a session wants."""
    ox, oy, _, _ = client_origin(hwnd)
    user32.SetCursorPos(int(ox + x), int(oy + y))
    time.sleep(0.15)
    user32.mouse_event(0x0002, 0, 0, 0, 0)
    time.sleep(0.05)
    user32.mouse_event(0x0004, 0, 0, 0, 0)


def post_click_client(hwnd, x, y):
    """Click without touching the real pointer, focus or z-order.

    The coordinates ride in the message itself, which is where winit reads a
    mouse input's position from -- so the shell sees a press and release at
    (x, y) and the pointer somebody else is using never moves, never hovers a
    different window and is never stolen mid-drag. The preceding move is what
    makes hover states real: this shell draws them from its own pointer
    tracking, and a press with no move before it arrives with no hover behind
    it.
    """
    packed = (int(y) << 16) | (int(x) & 0xFFFF)
    user32.PostMessageW(hwnd, WM_MOUSEMOVE, 0, packed)
    time.sleep(0.05)
    user32.PostMessageW(hwnd, WM_LBUTTONDOWN, MK_LBUTTON, packed)
    time.sleep(0.08)
    user32.PostMessageW(hwnd, WM_LBUTTONUP, 0, packed)
    # Leave the pointer where the click left it, so the frame that is captured
    # shows the pressed state resolved rather than a button held down.
    time.sleep(0.05)
    user32.PostMessageW(hwnd, WM_MOUSEMOVE, 0, packed)


def post_click(hwnd, x, y, send=False):
    """Press and release at client (x, y) with a message rather than the mouse.

    Which of `PostMessage` and `SendMessage` a window honours is not something a
    caller can know in advance: a message loop that reads the pointer's live
    position rather than the coordinates in the message will drop a posted click
    and answer one that is delivered on its own thread instead. The session
    script therefore lets each click say which it wants, and the answer is
    recorded rather than guessed at.
    """
    deliver = user32.SendMessageW if send else user32.PostMessageW
    packed = (int(y) << 16) | (int(x) & 0xFFFF)
    deliver(hwnd, WM_MOUSEMOVE, 0, packed)
    time.sleep(0.06)
    deliver(hwnd, WM_LBUTTONDOWN, MK_LBUTTON, packed)
    time.sleep(0.09)
    deliver(hwnd, WM_LBUTTONUP, 0, packed)
    time.sleep(0.06)
    deliver(hwnd, WM_MOUSEMOVE, 0, packed)


def tell_active(hwnd):
    """Tell the window it is active and focused without taking the foreground.

    Posted rather than set: `SetForegroundWindow` would pull the user out of
    whatever they are doing, while these two messages change nothing on screen
    and only alter what the window believes about itself -- which matters
    because an unfocused window is entitled to ignore a click.
    """
    user32.PostMessageW(hwnd, WM_ACTIVATE, 1, 0)
    time.sleep(0.05)
    user32.PostMessageW(hwnd, WM_SETFOCUS, 0, 0)
    time.sleep(0.05)


def post_key(hwnd, vk, send=False):
    """Press and release a virtual key, for the shortcuts the shell subscribes to."""
    deliver = user32.SendMessageW if send else user32.PostMessageW
    scan = user32.MapVirtualKeyW(vk, MAPVK_VK_TO_VSC)
    down = (scan << 16) | 1
    up = (scan << 16) | 1 | (1 << 30) | (1 << 31)
    deliver(hwnd, WM_KEYDOWN, vk, down)
    time.sleep(0.06)
    deliver(hwnd, WM_KEYUP, vk, up)
    time.sleep(0.06)


def park_offscreen(hwnd):
    """Move the window past the right edge of the desktop, without activating it.

    The same treatment `launch_check.py` gives a launch: the app still draws, its
    UI thread still pumps and `PrintWindow` still renders it, but it is not over
    anybody's work and it never takes the foreground, so a capture session on a
    machine in use is invisible rather than disruptive.
    """
    left, top, right, _ = window_rect(hwnd)
    width = max(400, right - left)
    x = user32.GetSystemMetrics(SM_XVIRTUALSCREEN) + user32.GetSystemMetrics(SM_CXVIRTUALSCREEN) + 200
    user32.SetWindowPos(hwnd, None, x, 8, width, 0,
                        SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE)
    left, top, _, _ = window_rect(hwnd)
    return left, top


def run_script(hwnd, path, method, click_settle):
    """Drive a capture session from a small script.

    Lines are `click X Y` (real pointer), `msgclick X Y` and `sendclick X Y`
(posted and delivered messages, no pointer), `key VK` and `sendkey VK`,
`shot OUT.png`, `activate`, `resize W H` and `wait SECONDS`; `#` comments and
blank lines are ignored. A session is worth scripting rather than
    re-launching per screenshot: a tab that has to be reached through four
    clicks would otherwise cost four app starts, and the first frame of a
    window is never the frame worth comparing.
    """
    shots = 0
    with open(path, "r", encoding="utf-8") as handle:
        for number, raw in enumerate(handle, 1):
            line = raw.split("#", 1)[0].strip()
            if not line:
                continue
            parts = line.split()
            verb = parts[0].lower()
            if verb == "click" and len(parts) == 3:
                post_click_client(hwnd, float(parts[1]), float(parts[2]))
                time.sleep(click_settle)
            elif verb in ("msgclick", "sendclick") and len(parts) == 3:
                post_click(hwnd, float(parts[1]), float(parts[2]), send=verb == "sendclick")
                time.sleep(click_settle)
            elif verb in ("key", "sendkey") and len(parts) == 2:
                post_key(hwnd, int(parts[1], 0), send=verb == "sendkey")
                time.sleep(click_settle)
            elif verb == "activate":
                tell_active(hwnd)
                time.sleep(click_settle)
            elif verb == "resize" and len(parts) == 3:
                user32.SetWindowPos(hwnd, None, 0, 0, int(parts[1]), int(parts[2]),
                                    SWP_NOZORDER | SWP_NOACTIVATE | SWP_NOMOVE)
                time.sleep(click_settle)
            elif verb == "shot" and len(parts) == 2:
                image, how = grab(hwnd, method)
                if image is None:
                    print(f"{path}:{number}: capture failed", file=sys.stderr)
                    return shots
                os.makedirs(os.path.dirname(os.path.abspath(parts[1])), exist_ok=True)
                image.save(parts[1])
                print(f"{parts[1]}: {image.width}x{image.height} via {how}")
                shots += 1
            elif verb == "wait" and len(parts) == 2:
                time.sleep(float(parts[1]))
            else:
                print(f"{path}:{number}: cannot read {line!r}", file=sys.stderr)
                return shots
    return shots


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
                        help="client-area X,Y to click before capturing; repeatable. "
                             "Moves the real pointer -- prefer --script")
    parser.add_argument("--script",
                        help="file of `click X Y`, `shot OUT.png` and `wait SECONDS` "
                             "lines, driven with posted messages and no cursor")
    parser.add_argument("--click-settle", type=float, default=1.2)
    parser.add_argument("--park", action="store_true",
                        help="move the window past the desktop edge as soon as it "
                             "appears, so a capture session never covers the screen")
    parser.add_argument("--method", choices=("auto", "print", "screen"), default="auto")
    parser.add_argument("--out", help="where to write the single capture")
    parser.add_argument("--keep", action="store_true",
                        help="leave a launched process running")
    args = parser.parse_args()
    if not args.out and not args.script:
        parser.error("one of --out or --script is required")

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
        # Wait for the *real* window, not the first thing the process puts on
        # screen. A launcher opens small helper windows -- an IME host, a hidden
        # 6x6 -- before its own, and `find_window` only prefers the largest among
        # whatever exists at that instant, so breaking on "something visible"
        # latches onto the helper and captures it.
        for _ in range(160):
            candidate = find_window(pid=proc.pid)
            if candidate:
                left, top, right, bottom = window_rect(candidate)
                if right - left >= MIN_WINDOW_WIDTH and bottom - top >= MIN_WINDOW_HEIGHT:
                    hwnd = candidate
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

    # Off the desk before anything else, not after the app has settled: the
    # window exists from the moment it is found, and six seconds of it sitting
    # over somebody's work is six seconds of exactly what `--park` is for.
    if args.park:
        left, top = park_offscreen(hwnd)
        print(f"parked at {left},{top}")

    time.sleep(args.settle)

    # Resolve again once the app has settled: the largest visible window of the
    # process is the one worth shooting, and by now there is no chance of
    # picking up a helper that appeared first.
    settled = find_window(pid=proc.pid) if proc is not None else find_window(title=args.title)
    if settled is not None and settled != hwnd:
        hwnd = settled
        if args.park:
            left, top = park_offscreen(hwnd)
            print(f"re-parked at {left},{top}")

    for click in args.click:
        x, y = (float(v) for v in click.split(","))
        click_client(hwnd, x, y)
        time.sleep(args.click_settle)

    if args.script:
        shots = run_script(hwnd, args.script, args.method, args.click_settle)
        ox, oy, cw, ch = client_origin(hwnd)
        print(f"{shots} shot(s); client {cw}x{ch} at screen {ox},{oy}")
        if shots == 0:
            return 3
    else:
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
