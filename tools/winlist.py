"""List the visible top-level windows, with their rectangles and processes.

Read-only: it enumerates windows and reports where they are, and changes nothing.
Written while diagnosing which window a capture attached to, so the answer is a
list rather than a guess.
"""

import ctypes
import ctypes.wintypes as wt

user32 = ctypes.WinDLL("user32", use_last_error=True)
kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)
WNDENUMPROC = ctypes.WINFUNCTYPE(ctypes.c_bool, wt.HWND, wt.LPARAM)

PROCESS_QUERY_LIMITED_INFORMATION = 0x1000
GW_OWNER = 4


def process_name(pid):
    handle = kernel32.OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, False, pid)
    if not handle:
        return "?"
    buffer = ctypes.create_unicode_buffer(260)
    size = wt.DWORD(260)
    kernel32.QueryFullProcessImageNameW(handle, 0, buffer, ctypes.byref(size))
    kernel32.CloseHandle(handle)
    return buffer.value.split("\\")[-1]


def window_title(hwnd):
    length = user32.GetWindowTextLengthW(hwnd)
    buffer = ctypes.create_unicode_buffer(length + 1)
    user32.GetWindowTextW(hwnd, buffer, length + 1)
    return buffer.value


def main():
    rows = []

    @WNDENUMPROC
    def visit(hwnd, _):
        if not user32.IsWindowVisible(hwnd) or user32.GetWindow(hwnd, GW_OWNER):
            return True
        title = window_title(hwnd)
        if not title:
            return True
        rect = wt.RECT()
        user32.GetWindowRect(hwnd, ctypes.byref(rect))
        pid = wt.DWORD()
        user32.GetWindowThreadProcessId(hwnd, ctypes.byref(pid))
        width = rect.right - rect.left
        height = rect.bottom - rect.top
        if width > 400 and height > 300:
            rows.append((rect.left, rect.top, width, height, process_name(pid.value), title[:70]))
        return True

    user32.EnumWindows(visit, 0)
    for left, top, width, height, process, title in sorted(rows, key=lambda r: (r[1], r[0])):
        print(f"x={left:>6} y={top:>6} {width:>5}x{height:<5} {process:<26} {title}")


if __name__ == "__main__":
    main()
