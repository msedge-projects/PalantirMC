# The reference client, measured off its own window

OWNS: `tools/refwalk.py`, `tools/refsample.py`, `tools/refocr.ps1`, `tools/refuia.ps1`,
`GATES.md`, `crates/palantir-desktop/src/**`

What the Modrinth App's window actually draws, read out of its pixels and its own
labels rather than from its source or from memory. This is the inventory the
shell is ported against, and `GATES.md` is where the numbers that matter become
commands.

The build measured is **0.204**, installed at
`%LOCALAPPDATA%\Modrinth App\Modrinth App.exe`, window pinned to a **1280x720
client area** so a capture of it and a capture of ours compare 1:1.

## How to re-measure it

```
# show the window: launched from a background shell it creates its Tauri window
# hidden and never shows it, so `show` is not optional
python tools/refwalk.py --attach "Modrinth App" --size 1296 728 1280 720

# a session is a script; every action is logged to .scratch/refwalk.log
python tools/refwalk.py --attach "Modrinth App" --script .scratch/ref-session-02.txt

# one region: its surfaces, its controls, its text lines and its labels
python tools/refsample.py .scratch/ref-01-home.png --region 981 48 1280 720
python tools/refsample.py .scratch/ref-01-home.png --ascii 0 4 360 44 --cols 120
```

`tools/refocr.ps1` reads labels out of a capture (Windows OCR, called by
`refsample.ocr`); it exists because the window's UI Automation tree stops at
`Modrinth App - Web content` — Chromium never turns its accessibility engine on
for a window nobody asks through its own host, so the tab names and button
captions cannot be read out of the control tree. `tools/refuia.ps1` is kept for
the parts that *are* exposed (the three panes, the window frame).

Captures live in `.scratch/` and are deliberately not committed: they are
screenshots of another product. The numbers below are the record.

## The shell

Three columns inside one 1280x720 client, each with its own 48px header, and a
1px `#42444a` rule under the header across the whole width.

| Column | Extent (client px) | Surface |
| --- | --- | --- |
| Icon rail | x 0..64 | `#27292e` — the raised chrome |
| Page pane | x 65..980 | `#16181c` — the page, one rounded top-left corner |
| Right panel | x 981..1280 | brand wash, `#172221` at the top to `#161d1e` at the bottom |

Header contents, left to right: the logo mark (28px, `#00da75`, x 8..36), the
wordmark "Modrinth" (x 41..159), an unidentified dim pair of hollow glyphs
(x 171..237, `#42444a`-range, unreadable by OCR — recorded, not guessed), then
the page's icon (16px, x 253..268) and its title (x 279.., 13px). The panel's
header carries its own label at x 980 ("No instances running") and a collapse
button at x 1250..1262.

## The rail

| Thing | Measured |
| --- | --- |
| Container | 64px wide, chrome `#27292e` |
| Entry | 48x48 at x 8..56, vertical pitch 52 (4px gap) |
| Active entry | plate `#1d563f`, radius ~11-12 (**not** a circle) |
| Active icon | `#00da75`, 22px |
| Idle icon | `#afbac4`, 22px |
| Divider | a short rule, y 582..586 in the 720px client |
| Bottom pair | 24px icon at y 624..648 (Settings), then a 12px element at y 682..694 |

Hovering each entry names it, which is how the destination list was read:

| y | Tooltip | Page it opens |
| --- | --- | --- |
| 76 | Home | Home |
| 124 | Discover content | Discover, with its own tab strip |
| 176 | Skin selector | Skin selector |
| 228 | Screenshots | Screenshots |
| 280 | Modrinth Hosting | Servers (shows "Modrinth App update required") |
| 636 | *(none read)* | Settings dialog |

A click at 688 while a dialog was open closed the dialog and changed no page, so
that element is not a navigation entry — recorded as unidentified, most likely
the account avatar or a footer control.

## The page column

| Thing | Measured |
| --- | --- |
| Pane | `#16181c`, top-left corner cut, content inset ~40px (x 105..929) |
| Page heading | white, 20px band, in-page at (88,77) on the Skin selector |
| Tab strip | 36px pills on the page's own `#16181c`, labels 13px white |
| Active tab | pill `#1d5540` — the same brand plate as the rail's active entry |
| Search field | `#34363c`, spans x 90..950, y ~130..170 (40px tall), 13px label |
| Sort / view | dropdown pills: `#34363c` fill, white labels, y 194 |
| Result card | `#27292e`, 106px tall, one per row across the full content width |
| Card icon | 40px tile, `#34363c` |

The Discover tab strip is **six** tabs, at y 89: Modpacks, Mods, Resource packs,
Data packs, Shaders, Servers.

## The right panel

| Thing | Measured |
| --- | --- |
| Width | 299px (x 981..1280) |
| Wash | `#172221` (top) to `#161d1e` (bottom) — straight, two stops |
| Cards | `#2a3633` with a `#3a4341` row (already asserted by `GATES.md` G6) |
| Promo cards | very dark green `#041209`/`#061d10` (Modrinth Hosting, Medal) |

## Tokens, as the running app has them

| Token | This app (0.204) | Ours before this change |
| --- | --- | --- |
| Brand / accent | `#00da75` | `#1bd96a` |
| Active plate | `#1d563f` (rail), `#1d5540` (tab) | `--` |
| Raised surface | `#27292e` | `#27292e` |
| Inset / input | `#34363c` | `#34363c` |
| Page | `#16181c` | `#16181c` |
| Divider | `#42444a` | `#42444a` |
| Idle rail icon | `#afbac4` | `#b0bac5` (one level off) |

The accent is the one real disagreement: our palette was transcribed from the
project's stylesheet ladder (`green-500`), and the app as installed paints
`#00da75` — the call-to-action button, the logo mark and the active rail icon
all measure it flat.

## Surfaces measured, with what they contain

- **Home** — hero "Welcome to Modrinth" / "Ready to start playing?" with a
  `#00da75` "+ Create an instance" button, "Press N to quick create an
  instance", and an "Escaping another launcher? / Import from launcher" row.
- **Discover** — six tabs, a search field, Sort and View dropdowns, and results
  as full-width 106px cards (icon, title, author, description, stats, loader
  chips, version, "Install").
- **Skin selector** — in-page heading, "Saved skins" and "Default skins" grids,
  "Add skin", and a drag-and-drop target.
- **Screenshots** — empty state "No screenshots yet" / "Screenshots you take
  in-game will appear here."
- **Servers** — "Modrinth App update required", "You need to update to use
  Modrinth Hosting through the Modrinth App", and a "Reload to update" button.
- **Create instance** (opened, captured, escaped) — three cards: "Custom setup"
  ("Start from scratch by picking a loader and game version"), a modpack choice
  ("Choose a project and we'll use its latest version"), and "Upload a modpack"
  ("Install a modpack from an .mrpack file on your device").
- **Settings** — three groups (DISPLAY, ACCOUNT, INSTANCES), eleven tabs,
  Appearance's "Color theme" cards (Dark / Light / OLED / Sync with system) and
  a "Sync theme across devices" toggle, footer "Modrinth APP 0.204" and the
  Windows build string.

## What this changes here

1. **The accent.** `#00da75` replaces `#1bd96a`, with hover and pressed derived
   by the rule the palette already documents (`brightness(1.25)` / `0.8`).
2. **The rail.** The active plate is a 12px-radius `#1d563f` square, not a
   circle, and the idle icon is `#afbac4`. The container is 64px.
3. **The order.** Home, Discover, Screenshots come first, in the reference's
   order and with its tooltips; the instance-scoped pages stay behind the
   divider, where the reference keeps its own instance content.
4. **The tab strip.** Modpacks leads, and the strip carries the reference's
   order for the types we serve.

## Still to port, and why each is not done here

- **The instance page's own tab strip** — the reference keeps Mods, Worlds,
  Files and Screenshots inside an instance, and the instance page is unreachable
  in this install because it has no instances. Creating one would write into the
  user's own Modrinth data; the walk was restricted to dialogs and Esc, so this
  surface is unmeasured rather than guessed at.
- **The Skin selector page** — needs a real skin source (offline skins, a
  username lookup, or a local file) before a page can honestly claim the name.
- **The Servers page** — the reference itself walls it behind an app update in
  0.204; there is no endpoint behind it to copy.
- **The create-instance chooser** — ours opens straight into the configure step;
  the reference opens with three cards. Porting it means reflowing the create
  flow, which is its own change.
- **The hover/press arithmetic** — `hover:brightness(1.25)` and
  `active:scale(0.95)` are the reference's, and a still capture cannot measure
  scale; recorded in `NEXT_STEPS.md` §12 as before.
