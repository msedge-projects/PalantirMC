"""Launch a staged exe off-screen, prove it came up, and read what it costs idle.

This is the half of verification CI cannot do. A runner has no desktop and no
window, so every build's *dynamic* behaviour -- does it start, is its UI thread
pumping, what does it hold while nothing is happening -- has to be checked on a
real machine, against an exe that was downloaded rather than compiled.

Nothing here touches the mouse. The window is moved beyond the primary desktop
with `SWP_NOACTIVATE` the moment it appears, so it never covers or focuses the
user's work, and `SendMessageTimeout(WM_NULL, SMTO_ABORTIFHUNG)` proves the UI
thread is pumping without activating anything. Parking it is safe to do: the
app persists only its colour theme (`prism-desktop`'s `Prefs`), so no window
position is written back that could send the next launch off-screen.

A series of windows is sampled rather than one, because startup work continues
past the first paint and a single window cannot tell the tail of that apart
from a process that is genuinely busy. Expect mostly zeros with the occasional
stray burst of tens of milliseconds; that is the shape seen so far, on both
Windows targets.

Usage:  python tools/launch_check.py <exe> [<exe> ...]
        LC_WINDOWS=8 ...        # sample more windows (default 6 of 1.5 s)
"""

import ctypes
import ctypes.wintypes as wt
import os
import subprocess
import sys
import time

user32 = ctypes.WinDLL("user32", use_last_error=True)
kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)
psapi = ctypes.WinDLL("psapi", use_last_error=True)

SWP_NOSIZE = 0x0001
SWP_NOZORDER = 0x0004
SWP_NOACTIVATE = 0x0010
WM_NULL = 0x0000
SMTO_ABORTIFHUNG = 0x0002
PROCESS_QUERY_LIMITED_INFORMATION = 0x1000
IDLE_SECONDS = 1.5
# A series rather than a single sample: the question is whether the background
# bursts after startup decay to nothing, or go on forever, and one window
# cannot tell those apart.
IDLE_WINDOWS = int(os.environ.get("LC_WINDOWS", "6"))


class PROCESS_MEMORY_COUNTERS(ctypes.Structure):
    _fields_ = [
        ("cb", wt.DWORD),
        ("PageFaultCount", wt.DWORD),
        ("PeakWorkingSetSize", ctypes.c_size_t),
        ("WorkingSetSize", ctypes.c_size_t),
        ("QuotaPeakPagedPoolUsage", ctypes.c_size_t),
        ("QuotaPagedPoolUsage", ctypes.c_size_t),
        ("QuotaPeakNonPagedPoolUsage", ctypes.c_size_t),
        ("QuotaNonPagedPoolUsage", ctypes.c_size_t),
        ("PagefileUsage", ctypes.c_size_t),
        ("PeakPagefileUsage", ctypes.c_size_t),
    ]


def find_window(pid, timeout=30.0):
    """The launcher's own top-level window, by title, owned by `pid`."""
    deadline = time.time() + timeout
    while time.time() < deadline:
        hwnd = user32.FindWindowW(None, "PalantirMC")
        if hwnd:
            owner = wt.DWORD()
            user32.GetWindowThreadProcessId(hwnd, ctypes.byref(owner))
            if owner.value == pid:
                return hwnd
        time.sleep(0.05)
    return None


def park_offscreen(hwnd):
    """Place the window beyond the primary desktop, without activating it."""
    rect = wt.RECT()
    user32.GetWindowRect(hwnd, ctypes.byref(rect))
    width = max(400, rect.right - rect.left)
    x = user32.GetSystemMetrics(0) + 200
    user32.SetWindowPos(hwnd, 0, x, 8, width, 0, SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE)
    after = wt.RECT()
    user32.GetWindowRect(hwnd, ctypes.byref(after))
    return after.left, after.top


def client_size(hwnd):
    rect = wt.RECT()
    user32.GetClientRect(hwnd, ctypes.byref(rect))
    return rect.right - rect.left, rect.bottom - rect.top


def cpu_seconds(handle):
    created, exited, kernel, user = (wt.FILETIME(), wt.FILETIME(), wt.FILETIME(), wt.FILETIME())
    if not kernel32.GetProcessTimes(
        handle, ctypes.byref(created), ctypes.byref(exited), ctypes.byref(kernel), ctypes.byref(user)
    ):
        raise ctypes.WinError(ctypes.get_last_error())

    def secs(ft):
        return ((ft.dwHighDateTime << 32) | ft.dwLowDateTime) / 1e7

    return secs(kernel) + secs(user)


def memory(handle):
    counters = PROCESS_MEMORY_COUNTERS()
    counters.cb = ctypes.sizeof(counters)
    if not psapi.GetProcessMemoryInfo(handle, ctypes.byref(counters), counters.cb):
        raise ctypes.WinError(ctypes.get_last_error())
    return counters.WorkingSetSize, counters.PeakWorkingSetSize


def settle(handle, quiet=0.35, ceiling=25.0):
    """Wait for startup work to stop, then return the cumulative CPU total."""
    started = time.time()
    last = cpu_seconds(handle)
    quiet_since = None
    while time.time() - started < ceiling:
        time.sleep(0.15)
        now = cpu_seconds(handle)
        if now - last < 0.0008:
            quiet_since = quiet_since or time.time()
            if time.time() - quiet_since >= quiet:
                return now
        else:
            quiet_since = None
        last = now
    return cpu_seconds(handle)


def responsive(hwnd):
    """Round-trip a no-op message through the UI thread; fails if it is hung."""
    result = ctypes.c_size_t()
    started = time.perf_counter()
    ok = user32.SendMessageTimeoutW(
        hwnd, WM_NULL, 0, 0, SMTO_ABORTIFHUNG, 2000, ctypes.byref(result)
    )
    return bool(ok), (time.perf_counter() - started) * 1000


def check(path):
    # CreateProcess needs a Windows path; a POSIX relative one is not found.
    exe = os.path.abspath(path)
    print(f"\n=== {exe}")
    proc = subprocess.Popen([exe], cwd=os.path.dirname(exe))
    try:
        started = time.perf_counter()
        hwnd = find_window(proc.pid)
        if not hwnd:
            raise SystemExit("  !! no window appeared within 30 s")
        appeared = time.perf_counter() - started
        x, y = park_offscreen(hwnd)

        handle = kernel32.OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, False, proc.pid)
        width, height = client_size(hwnd)
        alive_before = proc.poll() is None

        # A stricter quiet period than the A/B harness used: startup work here
        # continues past the first paint (instance scan, catalog), and a shorter
        # one mistakes that tail for idle cost.
        settled = settle(handle, quiet=1.0, ceiling=30.0)
        windows = []
        for _ in range(IDLE_WINDOWS):
            before = cpu_seconds(handle)
            time.sleep(IDLE_SECONDS)
            windows.append((cpu_seconds(handle) - before) * 1000)
        working, peak = memory(handle)
        total = sum(windows)
        busy = [i + 1 for i, w in enumerate(windows) if w >= 1.0]
        ok, round_trip = responsive(hwnd)
        alive_after = proc.poll() is None

        print(f"  window appeared in      : {appeared:6.2f} s")
        print(f"  client size             : {width}x{height}")
        print(f"  parked at x={x} (screen is {user32.GetSystemMetrics(0)} wide)")
        print(f"  alive on arrival / now  : {alive_before} / {alive_after}")
        print(f"  idle CPU, {IDLE_WINDOWS} x {IDLE_SECONDS} s windows:")
        for i, w in enumerate(windows):
            print(f"      window {i + 1:>2} after settle: {w:8.1f} ms")
        print(f"  total {IDLE_WINDOWS * IDLE_SECONDS:>4.1f} s idle       : {total:8.1f} ms"
              f"  ({total / (IDLE_WINDOWS * IDLE_SECONDS) / 10:.3f}% of one core)")
        print(f"  windows with any work   : {busy if busy else 'none'}")
        print(f"  working set             : {working / 1e6:6.1f} MB (peak {peak / 1e6:.1f} MB)")
        print(f"  UI thread responsive    : {ok} ({round_trip:.0f} ms round trip)")
        return dict(appeared=appeared, idle_ms=windows[-1], windows=windows,
                    working_mb=working / 1e6, peak_mb=peak / 1e6, responsive=ok,
                    alive=alive_after)
    finally:
        proc.terminate()
        try:
            proc.wait(timeout=10)
        except subprocess.TimeoutExpired:
            proc.kill()


if __name__ == "__main__":
    results = {}
    for path in sys.argv[1:]:
        results[path] = check(path)
        time.sleep(1.0)
    print("\n=== summary")
    for path, got in results.items():
        windows = ", ".join(f"{w:.1f}" for w in got["windows"])
        print(f"  {os.path.basename(path):24} up in {got['appeared']:.2f} s, "
              f"idle windows [{windows}] ms, {got['working_mb']:.1f} MB, "
              f"responsive={got['responsive']}")
