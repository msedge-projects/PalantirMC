# Dump the accessibility tree of a window as TSV.
#
# Why this exists: comparing this shell against the client it is modelled on
# needs that client's *inventory* -- every tab, button, checkbox and text field,
# with its role and where it sits -- and pixels alone cannot say what a rectangle
# is or what clicking it does. UI Automation can: WebView2 publishes the whole
# document tree, and each node answers for the patterns it supports, so "what
# this control does" is read out of the control rather than inferred from a
# screenshot. The captures then say what each node *looks* like, and the pair is
# what `REFERENCE.md` is written from.
#
# Read-only: it walks the tree and prints it. Nothing is clicked, focused,
# expanded or scrolled.
#
#   powershell -NoProfile -ExecutionPolicy Bypass -File tools/refuia.ps1 -Hwnd 123456
#   powershell -NoProfile -ExecutionPolicy Bypass -File tools/refuia.ps1 -Title "Modrinth App" -Out out.tsv

param(
    [int]$Hwnd = 0,
    [string]$Title = "",
    [int]$MaxDepth = 48,
    [string]$Out = ""
)

$ErrorActionPreference = "Stop"
Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes

function Find-Hwnd([string]$needle) {
    $found = @()
    foreach ($p in Get-Process | Where-Object { $_.MainWindowHandle -ne 0 }) {
        if ($p.MainWindowTitle -like "*$needle*") {
            $found += [pscustomobject]@{ h = [int64]$p.MainWindowHandle; t = $p.MainWindowTitle }
        }
    }
    if ($found.Count -eq 0) { return 0 }
    return ($found | Select-Object -First 1).h
}

if ($Hwnd -eq 0) {
    if (-not $Title) { Write-Error "one of -Hwnd or -Title is required"; exit 1 }
    $Hwnd = Find-Hwnd $Title
    if ($Hwnd -eq 0) { Write-Error "no window matching '$Title'"; exit 1 }
}

$root = [System.Windows.Automation.AutomationElement]::FromHandle([IntPtr]$Hwnd)
if (-not $root) { Write-Error "no automation element for hwnd $Hwnd"; exit 1 }

$walker = [System.Windows.Automation.TreeWalker]::ControlViewWalker

# One line per pattern the element answers, so a node's capabilities are facts
# rather than guesses. `TryGetCurrentPattern` on a pattern the element does not
# implement answers false instead of throwing, which is why it is used here
# rather than a try/catch around the property.
function Get-Patterns($el) {
    $names = @()
    $table = @(
        @("Invoke", [System.Windows.Automation.InvokePattern]::Pattern),
        @("Toggle", [System.Windows.Automation.TogglePattern]::Pattern),
        @("Select", [System.Windows.Automation.SelectionItemPattern]::Pattern),
        @("ExpandCollapse", [System.Windows.Automation.ExpandCollapsePattern]::Pattern),
        @("Value", [System.Windows.Automation.ValuePattern]::Pattern),
        @("RangeValue", [System.Windows.Automation.RangeValuePattern]::Pattern),
        @("Scroll", [System.Windows.Automation.ScrollPattern]::Pattern),
        @("Text", [System.Windows.Automation.TextPattern]::Pattern)
    )
    foreach ($row in $table) {
        $obj = $null
        if ($el.TryGetCurrentPattern($row[1], [ref]$obj)) { $names += $row[0] }
    }
    # Toggle/SelectionItem state, where the element has one: this is what says
    # whether a switch is on and which tab is the selected one.
    $state = ""
    $obj = $null
    if ($el.TryGetCurrentPattern([System.Windows.Automation.TogglePattern]::Pattern, [ref]$obj)) {
        $state = "on=" + ($obj.Current.ToggleState -eq [System.Windows.Automation.ToggleState]::On)
    }
    if ($el.TryGetCurrentPattern([System.Windows.Automation.SelectionItemPattern]::Pattern, [ref]$obj)) {
        $state += " selected=" + $obj.Current.IsSelected
    }
    $val = ""
    $obj = $null
    if ($el.TryGetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern, [ref]$obj)) {
        $val = $obj.Current.Value
    }
    return @(($names -join ","), $state, $val)
}

$lines = New-Object System.Collections.Generic.List[string]
$lines.Add("depth`tcut`tname`tid`tclass`tx`ty`tw`th`toffscreen`tenabled`tfocusable`tpatterns`tstate`tvalue")
$seen = New-Object 'System.Collections.Generic.HashSet[string]'

function Visit($el, $depth) {
    if ($depth -gt $MaxDepth) { return }
    $cur = $el.Current
    $key = "$($cur.ControlType.ProgrammaticName)|$($cur.Name)|$($cur.BoundingRectangle)"
    if (-not $seen.Add($key)) { return }
    $r = $cur.BoundingRectangle
    $cut = ($cur.ControlType.ProgrammaticName -replace '^ControlType\.', '')
    $name = ($cur.Name -replace "`t", " " -replace "`r?`n", " ")
    $p = Get-Patterns $el
    $lines.Add(("{0}`t{1}`t{2}`t{3}`t{4}`t{5}`t{6}`t{7}`t{8}`t{9}`t{10}`t{11}`t{12}`t{13}`t{14}" -f `
        $depth, $cut, $name, $cur.AutomationId, $cur.ClassName,
        [int]$r.X, [int]$r.Y, [int]$r.Width, [int]$r.Height,
        $cur.IsOffscreen, $cur.IsEnabled, $cur.IsKeyboardFocusable, $p[0], $p[1], $p[2]))
    $child = $walker.GetFirstChild($el)
    while ($child -ne $null) {
        Visit $child ($depth + 1)
        $child = $walker.GetNextSibling($child)
    }
}

Visit $root 0
if ($Out) { $lines | Set-Content -Encoding UTF8 $Out; Write-Host "wrote $($lines.Count - 1) nodes to $Out" }
else { $lines | ForEach-Object { Write-Host $_ } }
