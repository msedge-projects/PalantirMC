# Capturing both apps for a parity pass

`capture.py` takes one picture of one screen state, `pages.json` lists the
states, and `compose.py` -- someone else's -- puts two captures side by side.
Sift Launcher is being matched to the Modrinth desktop app pixel by pixel, and
two captures of one page are the only evidence that can settle it.

## What the machine needs

Windows 10 or 11, Python 3, and the Windows PowerShell already on it. Pillow is
**not** needed and must not become needed: the screen work is `System.Drawing`
in a helper `capture.py` writes to a temp directory and removes. The app must be
visible in an unlocked session, because the fallback grab copies the screen.

## The loop

Launch both apps, then, once per page:

    python tools/parity/capture.py --app Modrinth --page discover --out out
    python tools/parity/capture.py --app "Sift Launcher" --page discover --out out

...with `--page` set to `home`, `settings` and `discover-hover` in turn. Then
`python tools/parity/compose.py --dir out --all`, and **look at the composed
image**: list the differences in the order a person would notice them -- the
wrong grey, the box three pixels short, the icon at another weight, the
transition that lands late.

`--state` names the middle of the file, so a capture taken after a fix sits
beside the first instead of on it; `--settle-ms` (1200) is the wait after the
resize and again after the steps, and a page whose entrance animates wants more.

## What lands in `out/`

`<page>-<state>-<app>.png`, with `<app>` either `modrinth` or `sift`, plus one
line per capture in `out/report.txt`:

    app=sift page=discover state=idle rect=52,88,1440,900 corners=232,235,240;...

`rect` is the client area in screen pixels; its width and height are the PNG's
own. `corners` is the centre and the four inset corners, top-left first -- the
samples where two captures at different scroll or hover states disagree first.

## Tuning the coordinates

Every number in `pages.json` is a guess. Capture once with the app up, see where
the pointer landed, correct the constant, capture again; `constants` is a name
table, so a point two pages share is written once. A step that lands on a
control shows as that control's hover state in a capture that should be idle.

## What parity can and cannot mean

The two apps do not rasterise text identically: Modrinth is a WebView2 window on
the system's font settings, Sift is iced drawing its own glyph atlas, so stems
differ by a shade and a hairline box may move by a pixel. Parity is judged on
what both can agree on -- box positions, fills, borders, radii, type size,
weight and line height measured rather than read, icon shapes, transition
timing -- and text compared for metrics and colour, never glyph by glyph.

Exit codes: 0 captured, 1 a usage or `pages.json` problem, 2 no window title
matched `--app`, 3 the capture failed. A refused size warns and still captures.
