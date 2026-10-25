#!/usr/bin/env python3
"""Capture one screen state of a GUI application, on Windows.

Why this exists: this launcher is being brought to pixel parity with the
Modrinth desktop app, and parity is a claim about two pictures. Two pictures
need the same camera, so this tool pins the window's client area to a stated
size, drives it to a named state from `pages.json`, and writes one PNG and one
line of measurements into `--out` -- for both apps, from the same command shape:

    python tools/parity/capture.py --app Modrinth --page discover --out out
    python tools/parity/capture.py --app "Sift Launcher" --page discover --out out

What lands in the PNG is the **client area**, not the window frame: the frame is
the compositor's, it differs between an Electron window and an iced one by a few
pixels of DWM border, and neither app can do anything about it. What lands in
`report.txt` is where that client area sat on screen, so a reader holding two
files can tell "same page, same size" from "different page".

Windows only, deliberately. Going straight at `user32`/`gdi32` through ctypes is
the usual way and it means writing out DPI awareness, `INPUT` structs, and GDI+
lifetimes by hand, then writing a PNG encoder -- because there is no Pillow on
the capture machine. A `System.Drawing` snippet run by Windows PowerShell is the
same Win32 underneath and a hundred lines shorter, so all of Windows sits behind
one helper script and this file stays about *when* to click.

The helper runs one phase per process spawn:

* `attach` finds the window whose title contains `--app`, raises it, pins the
  client area to `--size`, and returns both rectangles. It exits 2 when nothing
  matches and names the titles that are open, because a typo in `--app` should
  cost one line to fix and not a session of guessing.
* `steps` runs the page's whole step list from one JSON payload: one spawn per
  page, not one per step. A spawn is an `Add-Type` compile, and thirty of them
  would make the pointer choreography slower than the transitions it is trying
  to photograph -- a hover captured through a slower path is a different picture.
* `capture` asks the window to render itself (`PrintWindow`, with
  `PW_RENDERFULLCONTENT` for Chromium's sake), falls back to copying the screen
  when that comes back blank, saves the PNG, then **reopens the file** to sample
  five pixels. Reopening is what makes the report line a statement about the
  saved image rather than about the bitmap it happened to come from.

`pages.json` holds the pages. A step is one of `{"move": [x, y]}`,
`{"click": [x, y]}`, `{"wheel": [x, y, notches]}`, `{"key": "ctrl+l"}` or
`{"wait": milliseconds}`, and coordinates are client-area pixels. Any `[x, y]`
may instead be `["NAME"]`, one entry of the file's `constants` table, so a point
two pages share is written down once and tuned in one place.

Every capture appends to `<out>/report.txt`:

    app=modrinth page=discover state=idle rect=52,88,1440,900 corners=...

`rect` is `left,top,width,height` of the client area in screen pixels; the width
and height are the PNG's own. `corners` is five samples -- the centre, then the
top-left, top-right, bottom-left and bottom-right insets -- as `r,g,b;...`.

Exit codes: 0 captured, 1 a usage or `pages.json` problem, 2 no window title
matched `--app`, 3 the capture failed. They are separate because the repair
differs: launch the app, fix the page file, or look at the machine.
"""
from __future__ import annotations

import argparse
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
DEFAULT_PAGES = HERE / "pages.json"

# The client area both apps are pinned to, so two captures share a grid. A window
# is pinned by its client area rather than its outer size because the outer size
# carries a frame the app does not draw.
DEFAULT_SIZE = "1440x900"

EXIT_OK = 0
EXIT_USAGE = 1
EXIT_NO_WINDOW = 2
EXIT_CAPTURE = 3

# How far in from each edge the four corner samples sit, as a fraction of the
# side. At 1440x900 this is 72px and 45px in: past any rounded window corner and
# any 1px border, and still inside the frame of the page.
CORNER_INSET = 0.05

# A helper that hangs must not hang the loop that takes eight captures. A page's
# own `wait` steps are inside this, so a page with more than two minutes of
# waits needs this raised.
HELPER_TIMEOUT = 120.0

# The Windows half, written to a temp file and run with `-File`. Inline because
# `-Command` would need a script this long re-quoted through PowerShell's own
# parser, and there is no third file to ship it in.
#
# Every phase prints one JSON object on stdout: the caller is a Python script
# and not a person reading a console, so the error text goes to stderr and the
# exit code says which kind of failure it was.
HELPER = r"""
param(
    [Parameter(Mandatory = $true)]
    [ValidateSet('attach', 'steps', 'capture')]
    [string] $Mode,
    [string] $Match = '',
    [int] $Width = 0,
    [int] $Height = 0,
    [string] $Steps = '',
    [long] $Hwnd = 0,
    [string] $Path = '',
    [string] $Points = ''
)

$ErrorActionPreference = 'Stop'
try {
    $utf8 = New-Object System.Text.UTF8Encoding $false
    [Console]::OutputEncoding = $utf8
} catch { }
$null = Add-Type -AssemblyName System.Drawing

if (-not ('ParityWin' -as [type])) {
    $source = @'
using System;
using System.Runtime.InteropServices;

public static class ParityWin
{
    [StructLayout(LayoutKind.Sequential)]
    public struct RECT { public int Left; public int Top; public int Right; public int Bottom; }

    [StructLayout(LayoutKind.Sequential)]
    public struct POINT { public int X; public int Y; }

    [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
    [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint pid);
    [DllImport("kernel32.dll")] public static extern uint GetCurrentThreadId();
    [DllImport("user32.dll")] public static extern bool AttachThreadInput(uint attach, uint to, bool join);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hWnd);
    [DllImport("user32.dll")] public static extern bool BringWindowToTop(IntPtr hWnd);
    [DllImport("user32.dll")] public static extern IntPtr SetActiveWindow(IntPtr hWnd);
    [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr hWnd, int command);
    [DllImport("user32.dll")] public static extern bool IsIconic(IntPtr hWnd);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr hWnd);
    [DllImport("user32.dll")] public static extern bool MoveWindow(IntPtr hWnd, int x, int y, int w, int h, bool repaint);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hWnd, out RECT rect);
    [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr hWnd, out RECT rect);
    [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr hWnd, ref POINT point);
    [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
    [DllImport("user32.dll")] public static extern void mouse_event(uint flags, uint dx, uint dy, int data, IntPtr extra);
    [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint flags, IntPtr extra);
    [DllImport("user32.dll")] public static extern IntPtr GetDC(IntPtr hWnd);
    [DllImport("user32.dll")] public static extern int ReleaseDC(IntPtr hWnd, IntPtr hdc);
    [DllImport("gdi32.dll")] public static extern bool BitBlt(IntPtr dst, int x, int y, int w, int h, IntPtr src, int srcX, int srcY, uint rop);
    [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr hWnd, IntPtr hdc, uint flags);
}
'@
    Add-Type -TypeDefinition $source -Language CSharp | Out-Null
}

# Physical pixels everywhere. Without this the process is DPI-virtualised: on a
# 125% display a request for a 1440px client area is answered with 1800 physical
# pixels, every click lands 25% off, and the PNG comes back the wrong size.
[void] [ParityWin]::SetProcessDPIAware()

# VK codes that Windows itself treats as extended; an app that reads the flag
# (any iced or Electron window) ignores a Delete that arrives without it.
$extendedKeys = @(0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27, 0x28, 0x2D, 0x2E)

function Get-ClientBox([IntPtr] $handle)
{
    $rect = New-Object ParityWin+RECT
    if (-not [ParityWin]::GetClientRect($handle, [ref] $rect)) { throw 'GetClientRect failed' }
    $origin = New-Object ParityWin+POINT
    $origin.X = 0
    $origin.Y = 0
    if (-not [ParityWin]::ClientToScreen($handle, [ref] $origin)) { throw 'ClientToScreen failed' }
    return @($origin.X, $origin.Y, ($rect.Right - $rect.Left), ($rect.Bottom - $rect.Top))
}

function Get-WindowBox([IntPtr] $handle)
{
    $rect = New-Object ParityWin+RECT
    if (-not [ParityWin]::GetWindowRect($handle, [ref] $rect)) { throw 'GetWindowRect failed' }
    return @($rect.Left, $rect.Top, ($rect.Right - $rect.Left), ($rect.Bottom - $rect.Top))
}

# Windows only honours SetForegroundWindow from the thread that already has the
# foreground. Joining that thread's input queue for the length of the call is the
# documented way round it, and it is left again immediately so the machine's own
# focus behaviour is not changed for longer than the raise.
function Show-Foreground([IntPtr] $handle)
{
    if ([ParityWin]::IsIconic($handle)) { [void] [ParityWin]::ShowWindow($handle, 9) }
    $foreground = [ParityWin]::GetForegroundWindow()
    $owner = [uint32] 0
    $foregroundThread = [ParityWin]::GetWindowThreadProcessId($foreground, [ref] $owner)
    $thisThread = [ParityWin]::GetCurrentThreadId()
    $joined = $false
    if ($foregroundThread -ne $thisThread) {
        $joined = [ParityWin]::AttachThreadInput($foregroundThread, $thisThread, $true)
    }
    [void] [ParityWin]::BringWindowToTop($handle)
    [void] [ParityWin]::SetForegroundWindow($handle)
    [void] [ParityWin]::SetActiveWindow($handle)
    if ($joined) {
        [void] [ParityWin]::AttachThreadInput($foregroundThread, $thisThread, $false)
    }
}

# MoveWindow sizes the *outer* window, so the frame is measured first and added
# to the client area that was asked for. Measuring rather than assuming is what
# makes 1440x900 mean 1440x900 on a window with a 31px title bar and an 8px
# invisible resize border.
function Set-ClientSize([IntPtr] $handle, [int] $clientWidth, [int] $clientHeight)
{
    $window = Get-WindowBox $handle
    $client = Get-ClientBox $handle
    $frameWidth = $window[2] - $client[2]
    $frameHeight = $window[3] - $client[3]
    $moved = [ParityWin]::MoveWindow(
        $handle, $window[0], $window[1], ($clientWidth + $frameWidth), ($clientHeight + $frameHeight), $true)
    if (-not $moved) { throw 'MoveWindow failed' }
}

function Move-Pointer([IntPtr] $handle, [int] $x, [int] $y)
{
    $point = New-Object ParityWin+POINT
    $point.X = $x
    $point.Y = $y
    if (-not [ParityWin]::ClientToScreen($handle, [ref] $point)) { throw "ClientToScreen failed for $x,$y" }
    if (-not [ParityWin]::SetCursorPos($point.X, $point.Y)) {
        throw "SetCursorPos failed for $($point.X),$($point.Y)"
    }
    # A press sent in the same millisecond as the move arrives with the move
    # still in the queue, and a row that highlights on hover never shows it.
    Start-Sleep -Milliseconds 60
}

function Get-VirtualKey([string] $token)
{
    $named = @{
        'ctrl' = 0x11; 'control' = 0x11; 'shift' = 0x10; 'alt' = 0x12; 'win' = 0x5B
        'enter' = 0x0D; 'return' = 0x0D; 'tab' = 0x09; 'esc' = 0x1B; 'escape' = 0x1B
        'space' = 0x20; 'backspace' = 0x08; 'delete' = 0x2E; 'del' = 0x2E; 'insert' = 0x2D
        'home' = 0x24; 'end' = 0x23; 'pageup' = 0x21; 'pagedown' = 0x22
        'left' = 0x25; 'up' = 0x26; 'right' = 0x27; 'down' = 0x28
    }
    $lower = $token.Trim().ToLowerInvariant()
    if ($named.ContainsKey($lower)) { return $named[$lower] }
    if ($lower -match '^f([1-9]|1[0-2])$') { return (0x6F + [int] $Matches[1]) }
    if ($token.Length -eq 1) {
        $character = [char]::ToUpperInvariant($token[0])
        if ([char]::IsLetterOrDigit($character)) { return [int] $character }
        $punctuation = @{ ',' = 0xBC; '.' = 0xBE; '/' = 0xBF; '-' = 0xBD; '=' = 0xBB; ';' = 0xBA; ' ' = 0x20 }
        if ($punctuation.ContainsKey([string] $token)) { return $punctuation[[string] $token] }
    }
    return -1
}

function Send-Combo([string] $combo)
{
    $tokens = @($combo.Split('+') | Where-Object { $_.Trim() -ne '' })
    if ($tokens.Count -eq 0) { throw 'empty key combination' }
    $keys = @()
    foreach ($token in $tokens) {
        $virtual = Get-VirtualKey $token
        if ($virtual -lt 0) { throw "unsupported key '$token' in '$combo'" }
        $keys += $virtual
    }
    foreach ($key in $keys) {
        $flags = 0
        if ($extendedKeys -contains $key) { $flags = 1 }
        [ParityWin]::keybd_event([byte] $key, 0, [uint32] $flags, [IntPtr]::Zero)
        Start-Sleep -Milliseconds 25
    }
    for ($i = $keys.Count - 1; $i -ge 0; $i--) {
        $flags = 2
        if ($extendedKeys -contains $keys[$i]) { $flags = 3 }
        [ParityWin]::keybd_event([byte] $keys[$i], 0, [uint32] $flags, [IntPtr]::Zero)
        Start-Sleep -Milliseconds 25
    }
}

# A window that cannot render itself answers `true` and hands back a bitmap of one
# colour -- transparent, or black. One colour over a 6x6 grid *and* one of those
# two colours is that answer, and nothing else is: measured on the capture machine,
# a successful PrintWindow of Notepad comes back as a uniform white client area
# inside a window with 45 distinct colours in it, so uniformity alone would have
# thrown away a good bitmap and gone to the screen for no reason.
function Test-Blank([System.Drawing.Bitmap] $bitmap)
{
    $first = $bitmap.GetPixel(0, 0).ToArgb()
    for ($row = 1; $row -lt 6; $row++) {
        for ($column = 1; $column -lt 6; $column++) {
            $x = [int] (($bitmap.Width - 1) * $column / 6)
            $y = [int] (($bitmap.Height - 1) * $row / 6)
            if ($bitmap.GetPixel($x, $y).ToArgb() -ne $first) { return $false }
        }
    }
    $black = [System.Drawing.Color]::Black.ToArgb()
    $clear = [System.Drawing.Color]::Transparent.ToArgb()
    return ($first -eq $black -or $first -eq $clear)
}

if ($Mode -eq 'attach') {
    $candidates = @(Get-Process -ErrorAction SilentlyContinue |
        Where-Object { $_.MainWindowHandle -ne 0 -and $_.MainWindowTitle -like "*$Match*" } |
        Sort-Object Id)
    if ($candidates.Count -eq 0) {
        $titles = @(Get-Process -ErrorAction SilentlyContinue |
            Where-Object { $_.MainWindowHandle -ne 0 -and $_.MainWindowTitle } |
            ForEach-Object { $_.MainWindowTitle } |
            Sort-Object -Unique)
        [Console]::Error.WriteLine(
            "no window title matched '*$Match*'. Windows that are open: " + ($titles -join ' | '))
        exit 2
    }
    $process = $candidates[0]
    $handle = [IntPtr] $process.MainWindowHandle
    Show-Foreground $handle
    if ($Width -gt 0 -and $Height -gt 0) {
        Set-ClientSize $handle $Width $Height
        # A resize can let the window manager pick its own top window again.
        Show-Foreground $handle
    }
    $client = Get-ClientBox $handle
    $window = Get-WindowBox $handle
    $exact = $true
    if ($Width -gt 0 -and $Height -gt 0) { $exact = ($client[2] -eq $Width -and $client[3] -eq $Height) }
    @{
        ok = $true
        hwnd = $handle.ToInt64()
        pid = $process.Id
        process = $process.ProcessName
        title = $process.MainWindowTitle
        window = $window
        client = $client
        requested = @($Width, $Height)
        exact = $exact
        visible = [ParityWin]::IsWindowVisible($handle)
    } | ConvertTo-Json -Compress -Depth 4
    exit 0
}

if ($Mode -eq 'steps') {
    $handle = [IntPtr] ([int64] 0)
    $index = 0
    try {
        $payload = Get-Content -Raw -LiteralPath $Steps | ConvertFrom-Json
        $handle = [IntPtr] ([int64] $payload.hwnd)
        foreach ($step in @($payload.steps)) {
            $properties = @($step.PSObject.Properties)
            if ($properties.Count -ne 1) {
                throw "has $($properties.Count) keys; each step is exactly one action"
            }
            $action = $properties[0].Name
            $value = $properties[0].Value
            switch ($action) {
                'move' {
                    Move-Pointer $handle ([int] $value[0]) ([int] $value[1])
                }
                'click' {
                    Move-Pointer $handle ([int] $value[0]) ([int] $value[1])
                    [ParityWin]::mouse_event(0x0002, 0, 0, 0, [IntPtr]::Zero)
                    Start-Sleep -Milliseconds 40
                    [ParityWin]::mouse_event(0x0004, 0, 0, 0, [IntPtr]::Zero)
                    Start-Sleep -Milliseconds 80
                }
                'wheel' {
                    $notches = [int] $value[2]
                    Move-Pointer $handle ([int] $value[0]) ([int] $value[1])
                    $direction = if ($notches -lt 0) { -1 } else { 1 }
                    # One event per notch: 360 in a single event is not a hand,
                    # and a scroll container that clamps a delta scrolls short.
                    for ($notch = 0; $notch -lt [Math]::Abs($notches); $notch++) {
                        [ParityWin]::mouse_event(0x0800, 0, 0, (120 * $direction), [IntPtr]::Zero)
                        Start-Sleep -Milliseconds 40
                    }
                }
                'key' {
                    Send-Combo ([string] $value)
                }
                'wait' {
                    Start-Sleep -Milliseconds ([int] $value)
                }
                default {
                    throw "unknown action '$action'"
                }
            }
            $index++
        }
    } catch {
        [Console]::Error.WriteLine('step ' + $index + ': ' + $_.Exception.Message)
        exit 1
    }
    @{ ok = $true; steps = $index } | ConvertTo-Json -Compress
    exit 0
}

if ($Mode -eq 'capture') {
    $handle = [IntPtr] $Hwnd
    $client = Get-ClientBox $handle
    $width = $client[2]
    $height = $client[3]
    if ($width -lt 8 -or $height -lt 8) {
        [Console]::Error.WriteLine("the client area is ${width}x${height}")
        exit 3
    }
    $window = Get-WindowBox $handle
    $offsetX = $client[0] - $window[0]
    $offsetY = $client[1] - $window[1]

    $bitmap = $null
    $full = $null
    $method = 'printwindow'
    try {
        $full = New-Object System.Drawing.Bitmap -ArgumentList $window[2], $window[3]
        $canvas = [System.Drawing.Graphics]::FromImage($full)
        $device = $canvas.GetHdc()
        $printed = [ParityWin]::PrintWindow($handle, $device, 2)
        $canvas.ReleaseHdc($device)
        $canvas.Dispose()
        if ($printed) {
            $frame = New-Object System.Drawing.Rectangle -ArgumentList $offsetX, $offsetY, $width, $height
            $bitmap = $full.Clone($frame, $full.PixelFormat)
        }
    } catch {
        $bitmap = $null
    } finally {
        if ($null -ne $full) { $full.Dispose() }
    }
    if ($null -ne $bitmap -and (Test-Blank $bitmap)) {
        $bitmap.Dispose()
        $bitmap = $null
    }

    if ($null -eq $bitmap) {
        # The screen path needs the window visible and the session unlocked,
        # which the raise above arranged; it is a fallback because it is the one
        # path that can photograph whatever else is on the machine's screen.
        $method = 'screen'
        $bitmap = New-Object System.Drawing.Bitmap -ArgumentList $width, $height
        $canvas = [System.Drawing.Graphics]::FromImage($bitmap)
        $device = $canvas.GetHdc()
        $screen = [ParityWin]::GetDC([IntPtr]::Zero)
        $copied = [ParityWin]::BitBlt(
            $device, 0, 0, $width, $height, $screen, $client[0], $client[1], 0x00CC0020)
        [void] [ParityWin]::ReleaseDC([IntPtr]::Zero, $screen)
        $canvas.ReleaseHdc($device)
        $canvas.Dispose()
        if (-not $copied) {
            $bitmap.Dispose()
            [Console]::Error.WriteLine('PrintWindow returned nothing and BitBlt failed')
            exit 3
        }
    }

    try {
        $bitmap.Save($Path, [System.Drawing.Imaging.ImageFormat]::Png)
    } catch {
        $bitmap.Dispose()
        [Console]::Error.WriteLine('could not write ' + $Path + ': ' + $_.Exception.Message)
        exit 3
    }
    $bitmap.Dispose()

    # Read the file back: the samples then describe the PNG that was written,
    # not the bitmap that was in memory when it was written.
    $saved = $null
    try {
        $loaded = [System.Drawing.Image]::FromFile($Path)
        $saved = New-Object System.Drawing.Bitmap -ArgumentList $loaded
        $loaded.Dispose()
    } catch {
        [Console]::Error.WriteLine('the PNG at ' + $Path + ' does not read back: ' + $_.Exception.Message)
        exit 3
    }

    $corners = @()
    if ($Points.Trim() -ne '') {
        foreach ($pair in $Points.Split(';')) {
            $parts = $pair.Split(',')
            $x = [Math]::Min([Math]::Max(0, [int] $parts[0]), $saved.Width - 1)
            $y = [Math]::Min([Math]::Max(0, [int] $parts[1]), $saved.Height - 1)
            $colour = $saved.GetPixel($x, $y)
            $corners += ,@([int] $colour.R, [int] $colour.G, [int] $colour.B)
        }
    }
    $imageWidth = $saved.Width
    $imageHeight = $saved.Height
    $saved.Dispose()

    @{
        ok = $true
        path = $Path
        method = $method
        client = $client
        image = @($imageWidth, $imageHeight)
        corners = $corners
    } | ConvertTo-Json -Compress -Depth 5
    exit 0
}

[Console]::Error.WriteLine("mode '$Mode' is not one of attach, steps, capture")
exit 1
"""


class Failure(Exception):
    """A reason to stop, carrying the exit code that says which kind of reason."""

    def __init__(self, code: int, message: str) -> None:
        super().__init__(message)
        self.code = code


class Parser(argparse.ArgumentParser):
    """argparse, with 1 for a usage error instead of argparse's own 2.

    2 means "no window title matched --app" in this tool, and a mistyped flag
    that also exited 2 would read as "the app is not running" and send the
    reader to look at the machine instead of at their command line.
    """

    def error(self, message: str) -> None:
        self.print_usage(sys.stderr)
        sys.stderr.write(f"{self.prog}: error: {message}\n")
        raise SystemExit(EXIT_USAGE)


def powershell() -> str:
    """The Windows PowerShell to run the helper with.

    `powershell.exe` from PATH first, because that name is in every shell a
    person is likely to have open; the System32 copy second, because a stripped
    PATH in a CI step still has the executable on disk.
    """
    found = shutil.which("powershell")
    if found:
        return found
    bundled = (
        Path(os.environ.get("SystemRoot", r"C:\Windows"))
        / "System32"
        / "WindowsPowerShell"
        / "v1.0"
        / "powershell.exe"
    )
    if bundled.exists():
        return str(bundled)
    raise Failure(EXIT_USAGE, "no powershell.exe on PATH or in System32; this tool needs Windows PowerShell")


def run_helper(helper: Path, mode: str, *rest: str) -> dict:
    """One PowerShell spawn, one JSON object back.

    A non-zero exit is the helper saying which kind of failure it hit -- 1 for a
    step it cannot perform, 2 for no matching window, 3 for a failed grab -- and
    it is carried out unchanged so the caller can say the same thing. Its stderr
    is the message: on a failed title match that text holds the window titles
    that *are* open, which is what turns a bad `--app` into a one-line repair.
    """
    command = [
        powershell(),
        "-NoProfile",
        "-NonInteractive",
        "-ExecutionPolicy",
        "Bypass",
        "-File",
        str(helper),
        "-Mode",
        mode,
        *rest,
    ]
    try:
        done = subprocess.run(
            command,
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="replace",
            timeout=HELPER_TIMEOUT,
        )
    except subprocess.TimeoutExpired:
        raise Failure(
            EXIT_CAPTURE, f"the {mode} helper did not answer within {HELPER_TIMEOUT:.0f}s"
        ) from None
    except OSError as exc:
        raise Failure(EXIT_USAGE, f"could not start PowerShell: {exc}") from None

    if done.returncode != 0:
        code = done.returncode if done.returncode in (EXIT_USAGE, EXIT_NO_WINDOW, EXIT_CAPTURE) else EXIT_CAPTURE
        message = (done.stderr or done.stdout).strip() or f"the {mode} helper exited {done.returncode}"
        raise Failure(code, message)

    # Windows PowerShell writes a BOM ahead of anything it is asked to print, and
    # json rejects it. It is an encoding artefact here, not a character.
    text = done.stdout.lstrip("\ufeff").strip()
    if not text:
        raise Failure(EXIT_CAPTURE, f"the {mode} helper printed nothing")
    try:
        return json.loads(text)
    except json.JSONDecodeError as exc:
        raise Failure(EXIT_CAPTURE, f"the {mode} helper printed no JSON ({exc}): {text[:200]}") from None


def load_pages(path: Path) -> tuple[dict[str, tuple[int, int]], dict]:
    """The constants table and the pages, from `pages.json`.

    Two shapes are accepted: the shipped `{"constants": {...}, "pages": {...}}`,
    and a bare page table at the top level, because tuning one page is easier in
    a trimmed copy than in the whole file. Keys beginning with `_` are comments
    and are skipped, which is how a JSON file gets to have any.
    """
    try:
        document = json.loads(path.read_text(encoding="utf-8-sig"))
    except FileNotFoundError:
        raise Failure(EXIT_USAGE, f"no such page file: {path}") from None
    except (json.JSONDecodeError, UnicodeDecodeError) as exc:
        raise Failure(EXIT_USAGE, f"{path} is not readable JSON: {exc}") from None
    if not isinstance(document, dict):
        raise Failure(EXIT_USAGE, f"{path}: expected an object at the top level")

    constants = document.get("constants", {})
    if not isinstance(constants, dict):
        raise Failure(EXIT_USAGE, f"{path}: `constants` must be an object")
    points: dict[str, tuple[int, int]] = {}
    for name, value in constants.items():
        if name.startswith("_"):
            continue
        whole = isinstance(value, list) and len(value) == 2
        if whole and all(isinstance(n, int) and not isinstance(n, bool) for n in value):
            points[name] = (int(value[0]), int(value[1]))
        else:
            raise Failure(EXIT_USAGE, f"{path}: constant `{name}` must be [x, y], got {value!r}")

    pages = document.get("pages")
    if pages is None:
        pages = {key: value for key, value in document.items() if key != "constants" and not key.startswith("_")}
    if not isinstance(pages, dict) or not pages:
        raise Failure(EXIT_USAGE, f"{path}: no pages in it")
    return points, pages


def page_spec(pages: dict, name: str, path: Path) -> dict:
    """One page's entry, named the way the caller named it."""
    if name not in pages:
        known = ", ".join(sorted(key for key in pages if not key.startswith("_")))
        raise Failure(EXIT_USAGE, f"{path}: no page `{name}`; it has {known}")
    spec = pages[name]
    if not isinstance(spec, dict):
        raise Failure(EXIT_USAGE, f"{path}: page `{name}` must be an object")
    return spec


def point_of(value, points: dict[str, tuple[int, int]], where: str) -> tuple[int, int]:
    """A constant's name as the point it stands for."""
    if isinstance(value, str):
        if value not in points:
            known = ", ".join(sorted(points)) or "none"
            raise Failure(EXIT_USAGE, f"{where}: no constant `{value}`; `constants` has {known}")
        return points[value]
    raise Failure(EXIT_USAGE, f"{where}: expected a constant name or [x, y], got {value!r}")


def pair_of(values, points: dict[str, tuple[int, int]], where: str) -> tuple[int, int]:
    """`["NAME"]` or `[x, y]` as two integers.

    One name rather than two numbers because the constants table holds points:
    `["RAIL_SETTINGS"]` is the whole target, and a second table of scalar x's and
    y's would be the same numbers written down twice.
    """
    if not isinstance(values, list) or not values:
        raise Failure(EXIT_USAGE, f'{where}: expected [x, y] or ["NAME"]')
    if len(values) == 1:
        return point_of(values[0], points, where)
    if len(values) == 2:
        for value in values:
            if isinstance(value, bool) or not isinstance(value, int):
                raise Failure(EXIT_USAGE, f"{where}: coordinates are whole pixels, got {value!r}")
        return (int(values[0]), int(values[1]))
    raise Failure(EXIT_USAGE, f'{where}: expected [x, y] or ["NAME"], got {len(values)} values')


def resolve_steps(spec: dict, points: dict[str, tuple[int, int]], where: str) -> list[dict]:
    """The page's steps as the helper wants them: one action per object, numbers.

    A step whose action is not one of the five is a typo in `pages.json`, and it
    stops the run before any window is touched. Skipping it would capture a
    state nobody asked for and file it under the name of one they did.
    """
    steps = spec.get("steps", [])
    if not isinstance(steps, list):
        raise Failure(EXIT_USAGE, f"{where}: `steps` must be a list")
    resolved = []
    for index, step in enumerate(steps):
        here = f"{where} step {index}"
        if not isinstance(step, dict) or len(step) != 1:
            raise Failure(EXIT_USAGE, f"{here}: expected one action per step, got {step!r}")
        action, value = list(step.items())[0]
        if action in ("move", "click"):
            x, y = pair_of(value, points, here)
            resolved.append({action: [x, y]})
        elif action == "wheel":
            if not isinstance(value, list) or len(value) < 2:
                raise Failure(EXIT_USAGE, f'{here}: wheel needs ["NAME", notches] or [x, y, notches]')
            notches = value[-1]
            if isinstance(notches, bool) or not isinstance(notches, int) or notches == 0:
                raise Failure(EXIT_USAGE, f"{here}: notches must be a non-zero whole number, got {notches!r}")
            x, y = pair_of(value[:-1], points, here)
            resolved.append({"wheel": [x, y, int(notches)]})
        elif action == "key":
            if not isinstance(value, str) or not value.strip():
                raise Failure(EXIT_USAGE, f'{here}: key needs a combination such as "ctrl+l"')
            resolved.append({"key": value})
        elif action == "wait":
            if isinstance(value, bool) or not isinstance(value, int) or value < 0:
                raise Failure(EXIT_USAGE, f"{here}: wait needs milliseconds, got {value!r}")
            resolved.append({"wait": int(value)})
        else:
            raise Failure(
                EXIT_USAGE, f"{here}: unknown action `{action}`; a step is move, click, wheel, key or wait"
            )
    return resolved


def parse_size(text: str) -> tuple[int, int]:
    """`1440x900` as two numbers, and `none` as "leave the window alone"."""
    folded = text.strip().lower()
    if folded in ("none", "keep", "off", "0x0"):
        return (0, 0)
    match = re.fullmatch(r"(\d+)\s*[x,]\s*(\d+)", folded)
    if not match:
        raise Failure(EXIT_USAGE, f"--size {text!r} is not WIDTHxHEIGHT")
    width, height = int(match.group(1)), int(match.group(2))
    if width < 320 or height < 240:
        raise Failure(EXIT_USAGE, f"--size {text!r} is smaller than any window this could be about")
    return (width, height)


def safe_name(value: str) -> str:
    """A page or state name that is safe in a filename, and says so if it changed."""
    cleaned = re.sub(r"[^A-Za-z0-9._-]+", "-", value.strip()).strip("-.")
    if not cleaned:
        raise Failure(EXIT_USAGE, f"{value!r} cannot name a file")
    if cleaned != value:
        print(f"capture.py: writing {value!r} as {cleaned!r}", file=sys.stderr)
    return cleaned


def app_slug(app: str) -> str:
    """The short name in the filename: `modrinth`, `sift`, or what was asked for.

    It is fixed at these two names because the paired comparison is a file-name
    join -- `<page>-<state>-modrinth.png` beside `<page>-<state>-sift.png` -- and
    a slug invented per invocation would silently stop pairing.
    """
    folded = app.strip().lower()
    for known in ("modrinth", "sift"):
        if known in folded:
            return known
    guess = re.sub(r"[^a-z0-9]+", "", folded)[:16]
    return guess or "app"


def sample_points(width: int, height: int) -> list[tuple[int, int]]:
    """Centre, then the four inset corners, in reading order.

    The inset is a fraction of the side rather than a fixed number of pixels, so
    the same five places are sampled at 1280x720 and at 1440x900. They are not
    meant to be interesting pixels; they are meant to catch the failure a
    thumbnail hides -- two captures of one page taken at different scroll or
    hover states disagree here long before they disagree anywhere visible.
    """
    inset_x = max(1, round(width * CORNER_INSET))
    inset_y = max(1, round(height * CORNER_INSET))
    right = width - 1 - inset_x
    bottom = height - 1 - inset_y
    return [
        (width // 2, height // 2),
        (inset_x, inset_y),
        (right, inset_y),
        (inset_x, bottom),
        (right, bottom),
    ]


def report_line(slug: str, page: str, state: str, shot: dict) -> str:
    """The one line per capture that `report.txt` gets."""
    left, top, width, height = shot["client"]
    corners = ";".join(
        ",".join(str(int(channel)) for channel in rgb) for rgb in shot["corners"]
    )
    return f"app={slug} page={page} state={state} rect={left},{top},{width},{height} corners={corners}"


def write_helper() -> Path:
    """The PowerShell helper on disk, in a directory of its own.

    Written with a BOM: Windows PowerShell reads a BOM-less script as ANSI, so one
    non-ASCII character in a comment would become a parse error rather than a
    comment. One directory per run, removed by the caller, because the step
    payload for the page goes in beside it.
    """
    directory = Path(tempfile.mkdtemp(prefix="sift-parity-"))
    helper = directory / "parity-win.ps1"
    helper.write_text(HELPER, encoding="utf-8-sig")
    return helper


def attach(helper: Path, app: str, width: int, height: int) -> dict:
    """Find, raise and pin the window; 2 if there is no such window."""
    window = run_helper(
        helper, "attach", "-Match", app, "-Width", str(width), "-Height", str(height)
    )
    if not window.get("ok"):
        raise Failure(EXIT_NO_WINDOW, f"no window matched {app!r}")
    return window


def drive(helper: Path, hwnd: int, steps: list[dict]) -> None:
    """Run the page's whole step list in one spawn.

    One process per page rather than one per step: every spawn pays an `Add-Type`
    compile, and thirty of them would put more latency between a `click` and the
    `wait` that is supposed to let the page finish moving than any transition
    needs, so the picture would depend on how fast this machine compiles C#.
    """
    payload = helper.parent / "steps.json"
    payload.write_text(json.dumps({"hwnd": int(hwnd), "steps": steps}), encoding="utf-8")
    run_helper(helper, "steps", "-Steps", str(payload))


def grab(helper: Path, window: dict, png: Path) -> dict:
    """Capture the client area, and refuse a file that does not describe it."""
    left, top, width, height = window["client"]
    points = ";".join(f"{x},{y}" for x, y in sample_points(width, height))
    shot = run_helper(
        helper,
        "capture",
        "-Hwnd",
        str(window["hwnd"]),
        "-Path",
        str(png),
        "-Points",
        points,
    )
    if not shot.get("ok") or not png.exists():
        raise Failure(EXIT_CAPTURE, f"no PNG at {png}")
    image = shot.get("image") or [0, 0]
    if (int(image[0]), int(image[1])) != (int(width), int(height)):
        raise Failure(
            EXIT_CAPTURE,
            f"the PNG is {image[0]}x{image[1]} but the client area is {width}x{height}",
        )
    corners = shot.get("corners") or []
    if len(corners) != 5 or any(len(rgb) != 3 for rgb in corners):
        raise Failure(EXIT_CAPTURE, f"expected five sampled corners of three channels, got {corners!r}")
    return shot


def parse_args(argv: list[str] | None) -> argparse.Namespace:
    parser = Parser(
        description="Capture one screen state of a running GUI app to a PNG, on Windows.",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""\
The PNG is the window's client area at --size, so two apps can be compared on
the same grid. Everything else is appended to <out>/report.txt as

  app=<slug> page=<page> state=<state> rect=<l,t,w,h> corners=<r,g,b;...>

where rect is the client area in screen pixels (its width and height are the
PNG's own) and corners is the centre of the image followed by its four inset
corners in reading order: top-left, top-right, bottom-left, bottom-right.

Steps come from pages.json: {"move": [x, y]}, {"click": [x, y]},
{"wheel": [x, y, notches]}, {"key": "ctrl+l"}, {"wait": milliseconds}, with
coordinates in client-area pixels. Any [x, y] may be ["NAME"], one point from
that file's `constants` table.

Exit codes: 0 captured, 1 usage or pages.json, 2 no window matched --app,
3 the capture failed.
""",
    )
    parser.add_argument("--app", required=True, help='a substring of the window title, e.g. "Modrinth"')
    parser.add_argument("--out", required=True, help="directory for the PNG and report.txt; created if missing")
    parser.add_argument("--page", required=True, help="which entry of pages.json to drive, e.g. discover")
    parser.add_argument("--state", default="idle", help="a name for this capture of the page (default: idle)")
    parser.add_argument(
        "--settle-ms",
        type=int,
        default=1200,
        help="milliseconds to wait after the resize and again after the steps (default: 1200)",
    )
    parser.add_argument(
        "--size",
        default=DEFAULT_SIZE,
        help=f"client area to pin, WIDTHxHEIGHT (default: {DEFAULT_SIZE}); 'none' leaves the window as it is",
    )
    parser.add_argument(
        "--pages", default=str(DEFAULT_PAGES), help="the page table (default: pages.json beside this file)"
    )
    return parser.parse_args(argv)


def capture(args: argparse.Namespace) -> int:
    """The whole capture, in the order the machine needs it to happen."""
    if sys.platform != "win32":
        raise Failure(
            EXIT_USAGE,
            "capture.py drives user32 through Windows PowerShell; on another system "
            "there is no window to find",
        )
    if not args.app.strip():
        raise Failure(EXIT_USAGE, "--app needs a substring of a window title")
    if args.settle_ms < 0:
        raise Failure(EXIT_USAGE, "--settle-ms cannot be negative")

    width, height = parse_size(args.size)
    pages_path = Path(args.pages)
    points, pages = load_pages(pages_path)
    spec = page_spec(pages, args.page, pages_path)
    steps = resolve_steps(spec, points, args.page)

    page = safe_name(args.page)
    state = safe_name(args.state)
    slug = app_slug(args.app)
    if slug not in ("modrinth", "sift"):
        print(
            f"capture.py: --app {args.app!r} is neither app, so the file is named {slug!r} "
            "and will not pair with a Modrinth or Sift capture",
            file=sys.stderr,
        )

    out = Path(args.out).expanduser()
    out.mkdir(parents=True, exist_ok=True)
    png = out / f"{page}-{state}-{slug}.png"

    helper = write_helper()
    try:
        window = attach(helper, args.app, width, height)
        if not window.get("visible", True):
            print(f"capture.py: {window.get('title')!r} is not visible; the fallback grab will miss", file=sys.stderr)
        if not window.get("exact", True):
            left, top, actual_width, actual_height = window["client"]
            print(
                f"capture.py: the client area is {actual_width}x{actual_height}, not {width}x{height} "
                "(a minimum size, or a maximised window); this capture cannot be compared to one "
                "taken at another size",
                file=sys.stderr,
            )
        # The resize is settled first: an Electron window lays out on its next
        # few frames, and a click sent before that lands on the old layout.
        time.sleep(args.settle_ms / 1000.0)
        if steps:
            drive(helper, window["hwnd"], steps)
            # And again after the steps, which is what lets a hover transition
            # or a route change finish before the picture is taken.
            time.sleep(args.settle_ms / 1000.0)
        shot = grab(helper, window, png)
    finally:
        shutil.rmtree(helper.parent, ignore_errors=True)

    line = report_line(slug, page, state, shot)
    with (out / "report.txt").open("a", encoding="utf-8", newline="\n") as report:
        report.write(line + "\n")

    left, top, _, _ = shot["client"]
    print(f"{png}  {shot['image'][0]}x{shot['image'][1]} at {left},{top}  via {shot.get('method')}")
    hint = spec.get("title_hint")
    if isinstance(hint, str) and hint:
        print(f"expect the page's own heading to read {hint!r}")
    if steps:
        print(f"{len(steps)} steps ran; the reported corners are {line.split('corners=')[1]}")
    return EXIT_OK


def main(argv: list[str] | None = None) -> int:
    args = parse_args(argv)
    try:
        return capture(args)
    except Failure as failure:
        print(f"capture.py: {failure}", file=sys.stderr)
        return failure.code


if __name__ == "__main__":
    sys.exit(main())
