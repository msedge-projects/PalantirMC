# Read the text out of a capture, with where each word sits.
#
# Why this exists: the reference client is a WebView2 window, and its UIA tree
# stops at "Modrinth App - Web content" -- Chromium never turns its accessibility
# engine on for a window nobody is asking for through its own host. So the labels
# a clone has to copy (rail entries, tab names, button captions, settings rows)
# cannot be read out of the control tree. They can be read out of the pixels, and
# Windows ships an OCR engine, so the inventory is: geometry from the capture
# (tools/refsample.py), labels from this.
#
#   powershell -NoProfile -ExecutionPolicy Bypass -File tools/refocr.ps1 -Image shot.png
#   powershell ... -File tools/refocr.ps1 -Image shot.png -Tsv out.tsv
#
# Output is TSV: x, y, w, h, then the recognised line. Coordinates are relative to
# the image, left to right, top to bottom.
#
# Windows PowerShell 5.1 projects a WinRT `IAsyncOperation` as a bare
# __ComObject with no GetAwaiter, so every async call here goes through
# WindowsRuntimeSystemExtensions.AsTask via the reflection shim below. That shim
# is the whole reason this is a script rather than three lines.

param(
    [Parameter(Mandatory = $true)][string]$Image,
    [string]$Tsv = "",
    [string]$Language = ""
)

$ErrorActionPreference = "Stop"

$null = [Windows.Media.Ocr.OcrEngine, Windows.Foundation, ContentType = WindowsRuntime]
$null = [Windows.Graphics.Imaging.BitmapDecoder, Windows.Foundation, ContentType = WindowsRuntime]
$null = [Windows.Graphics.Imaging.SoftwareBitmap, Windows.Foundation, ContentType = WindowsRuntime]
$null = [Windows.Storage.StorageFile, Windows.Foundation, ContentType = WindowsRuntime]
$null = [Windows.Storage.FileAccessMode, Windows.Foundation, ContentType = WindowsRuntime]
Add-Type -AssemblyName System.Runtime.WindowsRuntime

$script:AsTask = ([System.WindowsRuntimeSystemExtensions].GetMethods() | Where-Object {
        $_.Name -eq 'AsTask' -and
        $_.GetParameters().Count -eq 1 -and
        $_.GetParameters()[0].ParameterType.Name -eq 'IAsyncOperation`1'
    })[0]

function Await($operation, [Type]$resultType) {
    $task = $script:AsTask.MakeGenericMethod($resultType).Invoke($null, @($operation))
    $task.Wait(-1) | Out-Null
    return $task.Result
}

$path = (Resolve-Path $Image).Path
$file = Await ([Windows.Storage.StorageFile]::GetFileFromPathAsync($path)) ([Windows.Storage.StorageFile])
$stream = Await ($file.OpenAsync([Windows.Storage.FileAccessMode]::Read)) ([Windows.Storage.Streams.IRandomAccessStream])
$decoder = Await ([Windows.Graphics.Imaging.BitmapDecoder]::CreateAsync($stream)) ([Windows.Graphics.Imaging.BitmapDecoder])
$bitmap = Await ($decoder.GetSoftwareBitmapAsync()) ([Windows.Graphics.Imaging.SoftwareBitmap])

# The engine takes Bgra8 or Gray8 only, and a PNG capture comes back in whatever
# the window was drawn in (BGRA straight from PrintWindow, but RGBA from some
# paths), so the format is forced rather than hoped for.
if ($bitmap.BitmapPixelFormat -ne [Windows.Graphics.Imaging.BitmapPixelFormat]::Bgra8) {
    $bitmap = Await ([Windows.Graphics.Imaging.SoftwareBitmap]::ConvertAsync(
            $bitmap, [Windows.Graphics.Imaging.BitmapPixelFormat]::Bgra8)) ([Windows.Graphics.Imaging.SoftwareBitmap])
}

if ($Language) {
    $lang = New-Object Windows.Globalization.Language $Language
    $engine = [Windows.Media.Ocr.OcrEngine]::TryCreateFromLanguage($lang)
} else {
    $engine = [Windows.Media.Ocr.OcrEngine]::TryCreateFromUserProfileLanguages()
}
if (-not $engine) {
    Write-Error "no OCR engine for the requested language"
    exit 1
}

$result = Await ($engine.RecognizeAsync($bitmap)) ([Windows.Media.Ocr.OcrResult])

$lines = New-Object System.Collections.Generic.List[string]
foreach ($line in $result.Lines) {
    $xs = @(); $ys = @(); $rs = @(); $bs = @()
    foreach ($word in $line.Words) {
        $r = $word.BoundingRect
        $xs += [int]$r.X; $ys += [int]$r.Y; $rs += [int]($r.X + $r.Width); $bs += [int]($r.Y + $r.Height)
    }
    $x = ($xs | Measure-Object -Minimum).Minimum
    $y = ($ys | Measure-Object -Minimum).Minimum
    $w = ($rs | Measure-Object -Maximum).Maximum - $x
    $h = ($bs | Measure-Object -Maximum).Maximum - $y
    $text = ($line.Text -replace "`t", " " -replace "`r?`n", " ")
    $lines += ("{0}`t{1}`t{2}`t{3}`t{4}" -f $x, $y, $w, $h, $text)
}

if ($Tsv) {
    $lines | Set-Content -Encoding UTF8 $Tsv
    Write-Host "read $($lines.Count) lines from $Image into $Tsv"
} else {
    $lines | ForEach-Object { Write-Host $_ }
}
