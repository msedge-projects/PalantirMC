# Parity log — Sift Launcher against the Modrinth app

Every entry is a page and state, the pair of captures it was judged from, and the
differences found by **looking at them side by side**. A page is done when its list is
empty and the pair is attached to the commit that emptied it.

The reference is the **Modrinth app v0.22.0** (`Modrinth.App_0.22.0_x64-setup.exe`),
running on the `sift-launcher` VM in a container because its Linux build needs glibc
2.43 and the VM has 2.41 (see `docs/PLAN-SIFT.md` §2).

Captures: `tools/parity/capture_x11.py` (VM, X11) and `tools/parity/capture.py`
(Windows). Both write the same report line, and `tools/parity/compose.py` pairs them.

---

## discover / idle — 2026-10-25, first pair

Captures: 1440x900 both, window rect `0,0,1440,900` both.
Report: centre and three of four corners identical (`39,41,46`); bottom-right differs
(`2,39,18` on Modrinth, `22,24,28` on ours) — that corner lands on the rail's tail in
one and empty space in the other, so it is a window-composition difference, not a
colour difference. **Needs a real measurement, not a corner sample.**

### Matches (by eye, at 1:1 scale)

| Region | What matches |
| --- | --- |
| Left rail | Icon order and glyphs: discover, search, library, folder, server, then a gap, then the settings/reload group. Same icons, same vertical order. |
| Header | Brand mark + name at left, breadcrumb, centred nothing, `No instances running` at right, then window controls. Same pieces in the same order. |
| Tabs | `Mods`, `Resource Packs`, `Data Packs`, `Shaders`, `Servers` in the same order with the same active state. |
| Result cards | Same cards, same order (Fabulously Optimized, Zombie Invade, Cobblemon Official, COBBLEVERSE), same badges, same action buttons at the same corner. |
| Right panel | Same sections: `Getting started`, `Hide already installed`, Modrinth news card. |

### Differences found (all open)

| # | Region | What differs | Suspected cause | How to confirm |
| --- | --- | --- | --- | --- |
| OBS-1 | Right panel | A stray `1 Instance / Minecraft / Modrinth` card floats over our panel, and the panel's text column is clipped at the window edge | the pointer was still over an instance chip when captured (a hover card), **or** our panel is wider than the reference's | capture again with the pointer parked in empty header space; then measure the panel's right edge |
| OBS-2 | Tabs | Our tab labels look a touch heavier and sit ~1-2 px lower; the active underline looks wider | font weight or the underline's padding | crop both tab strips at 3x and overlay |
| OBS-3 | Search field | Ours appears ~80 px narrower than the reference's | the field's flex basis against the sort/view controls | measure both fields' left/right edges in the captures |
| OBS-4 | Right panel | `Getting started` rows' vertical rhythm looks slightly tighter in ours | row gap token | measure row pitch |
| OBS-5 | Card | The card's corner radius and border tone need a pixel check | radius token | sample the same corner point on both |

### What this entry is not

It is not a verdict on any of the five. Every row is a hypothesis from one 1500 px-wide
composite; the next pass crops each region at 1:1 and measures edges rather than
eyeballing them, and OBS-1 in particular may vanish once the pointer is parked.

---

## How to add an entry

1. `python3 tools/parity/capture_x11.py --app "Modrinth App" --page <page> --out out`
2. `python3 tools/parity/capture_x11.py --app PalantirMC --page <page> --out out`
3. `python3 tools/parity/compose.py --dir out --page <page> --state idle`
4. **Open** `out/<page>-idle-compare.png`, list every difference in the table above,
   with a region, what differs, the suspected cause and how to confirm it.
5. Fix, re-capture, and delete rows that no longer reproduce. Attach the final pair.

---

## Measured reference geometry (v0.22.0, 1440x900, discover idle)

Read with `tools/parity/edges.py` from `discover-idle-modrinth.png`. These are the
numbers the port should be built against, not eyeballed ones; every row is a window
row and every value an x where a channel moved by more than 10.

| Row | Edges found | Reading |
| --- | --- | --- |
| 20 (title/header) | 182 | dense text region: brand mark at ~12-32, then the breadcrumb; not a layout boundary row |
| 105 (tab strip) | 23 | rail ends **x=64**; content column begins **x=88** (`88,89`); tab container inner edge at 96; first tab label at **196**; a container edge at **721** |
| 150 (search row) | 224 | rail ends 64; content begins **88**; text runs from 107 |

**What this already says about OBS-3.** The content column starts at x=88 in the
reference. If ours starts at the same place, then the narrower field is not the column's
left edge but its **right** edge (or the space the sort/view controls take), which is the
next thing to measure: the same rows on `discover-idle-sift.png`.

Not yet measured, and needed before the chrome is written: the content column's right
edge, the right panel's left edge and width, the tab strip's height, the rail's icon
pitch (the first two centres), and the title bar's height. They are one `edges.py` run
each; the reason they are not in this table is that the loop was still being built when
the table was written.

---

## The egui shell's first frame — 2026-10-25

The rewritten shell renders: title bar with the accent plate and `Sift Launcher`, the five
destination rail with its glyphs, the tab strip, twelve cards through fastframe's scrolling,
and the right panel's `Getting started` rows. Its structure is the reference's; what is
left is convergence, and these are the first differences seen at 1:1 against Modrinth
v0.22.0 side by side:

| # | Region | What differs |
| --- | --- | --- |
| E-1 | Header | the reference has a page header (breadcrumb `Discover modpacks` with back/forward arrows) between the title bar and the tabs; ours goes straight from bar to tabs |
| E-2 | Title bar | ours carries the mark and the name only; the reference also carries `No instances running` and the window controls |
| E-3 | Tabs | the reference's strip is inset inside the content column with its own rounded container; ours spans the column edge to edge |
| E-4 | Rail | our rail has no utility group (settings, reload, account) and no separator between the groups |
| E-5 | Right panel | ours draws three rows; the reference has three rows, a `Hide already installed` toggle and the news card, with the toggle's own switch drawn |
| E-6 | Cards | ours are placeholders: no icon, no install button, no badges, no footer stats |
| E-7 | Backgrounds | the content column looks a shade lighter than the reference's; needs a sampled comparison rather than an eye |

### Second pass: E-1 to E-5 closed, and what the 2x crops found

The chrome now matches the reference where the first pass did not, and the way it was
found is worth recording because two of the five "differences" were the wrong diagnosis:

| Was | Actually |
| --- | --- |
| E-1 "missing page header" | there is no separate header row: the back/forward arrows, the route badge and the page's name sit **on the title bar's own 40 px line**, to the right of the brand |
| E-3 "tab strip spans the column" | the strip is a rounded container of **six** pills, `Modpacks` first and drawn as the active green pill — the count was wrong, not just the inset |
| E-2, E-4, E-5 | confirmed as diagnosed: title-bar furniture, rail utility group below its rule, the panel's toggle and news card |

All five are drawn now, from constants in `crates/sift-ui/src/geometry.rs`, and the tab
labels, breadcrumb, card titles and panel headings are set in **Inter at the reference's
weights** (400/500/600/700/800 as separate families), which is what made the strip stop
looking "lighter and smaller": egui ships one proportional face, so without registering the
weights every heading renders at 400.

### Third pass: E-6 closed, and the CDN taught two lessons

The card now carries the reference's anatomy and content: a 64-point icon tile, the title
with its author and link glyph on one baseline, a summary wrapped at the reference's own
measure (both cards break after "in a"), a row of tag pills with their glyphs, the three
stats right-aligned on that row, and the `Install` button at the top-right. The card is 148
tall -- the 2.2x crop said so, after the first guess of 116.

Two things about icons that only a live run could have found:

| Assumption | Measured |
| --- | --- |
| Icons live at `https://cdn.modrinth.com/data/<id>/icon.png` | **400 on every id.** The API's own `icon_url` is a hashed `.../<sha>_96.webp`, and some projects still serve `.png` |
| PNG and JPEG decoders are enough | **WebP is the common case**, so a decoder without it turns every icon into a grey tile |

Both are now constants in the sample data with the reason beside them, and the image
dependency names the three formats it needs and says why each is there.

| # | Region | What differs |
| --- | --- | --- |
| E-11 | Card tile | the icon is drawn into a square; the reference's 64-point tile is rounded, so a light icon shows square corners against the plate |
| E-12 | Card | the reference's title row carries a small follower glyph on hover, and its `Install` button has a border rather than a fill |

### Home, first pair — 2026-10-25

The app opens on Home like the reference: the rail's first destination active, the breadcrumb
`Home`, the centred hero (`Welcome to Sift Launcher`, `Ready to start playing?`, the green
`+ Create an instance`, the `N` hint) and the `Escaping another launcher?` prompt pinned near
the bottom.

**The background matches exactly, and the eye was wrong about it.** A side-by-side composite
made our window look lighter, so the pixels were sampled instead of argued with:

| Point | Reference | Ours |
| --- | --- | --- |
| content (700,400) | `srgb(22,24,28)` | `srgb(22,24,28)` |
| rail (40,400) | `srgb(39,41,46)` | `srgb(39,41,46)` |
| panel (1250,400) | `srgb(44,81,63)` | `srgb(39,41,46)` |

Two lessons, both cheap: a downscaled JPEG shifts perceived lightness enough to fake a theme
bug, and the one point that *did* differ says something real -- the reference's panel has the
news card's green art there and ours has plain panel.

| # | Region | What differs |
| --- | --- | --- |
| E-13 | Right panel (all pages) | the reference's panel carries `News` with a card -- art, title `Sync settings across instances`, two lines of copy, a date and `Upgrade to Modrinth+` -- and a hosting banner below it; ours has the news heading and one line |
| E-14 | Home hero | the mark plate is empty: the brand is `logo512.png`, uploaded as a texture in the slice that does it |
| E-15 | Rail | the reference's active plate is a rounded square with the glyph inset; ours is close but the icon is 20 where the reference's is 22 |

**A rig bug worth the paragraph:** the first Home capture came back showing *this launcher*
instead of the reference. The capture reads the screen and crops, and both windows sit at
0,0 at the same size, so whatever is on top is what gets photographed -- `windowactivate`
promises nothing about stacking on a bare X server with no window manager. The tool now
raises the target, lowers every other named window and raises the target again.

### Fourth pass: the rail was invented, and the icons became exact

**A capture found a structural error nobody's plan had.** Clicking the reference's third rail
slot landed on its *skin selector*, not on an instances page. The reference's rail is:

```
Home   Discover   Skins   Screenshots   Servers
```

There is no `Library` and no `Files` item -- both were mine, invented when the rail was first
drawn. The shell now carries the reference's five in its order, and the Library page survives
as a page reached from Home, which is where the reference lists instances. This is the difference
a side-by-side catches and a spec does not: the rail looked plausible, and it was wrong.

**The icon set is now geometrically exact where epaint allows it.** The generator ear-clips
filled contours with hole bridging and emits a `Mesh`, and inserts a disc at every round join and
cap:

| | before | after |
| --- | --- | --- |
| shapes | 1,734 | **3,146** |
| points | 24,183 | 58,805 |
| mesh triangles | 0 | **27,868** |
| round pieces | 0 | 4,628 (2,226 joins + 2,402 caps) |
| generated source | 1.05 MB | 2.68 MB |

Every one of the 16 previously-wrong glyphs now reports a mesh area *smaller* than its outer
contour alone -- `cog` 180.9 < 209.0, `radio-button` 112.9 < 313.7, `potato` 48,757 < 207,472 --
which is the hole being subtracted, checked on every generation rather than once.

| # | Region | What differs |
| --- | --- | --- |
| E-16 | Icons | epaint does not feather raw meshes, so filled glyphs and the round pieces have hard edges where a `PathShape` fill had ~0.5 px of anti-aliasing. MSAA would restore it; strokes still feather either way |
| E-17 | Icons | the miter tip sits under each join disc rather than being replaced: a 90-degree corner's miter reaches 1.41r and a shape cannot erase it |
| E-18 | Icons | round caps are applied to every stroke, though 28 glyphs ask for butt or miter in their SVG; the iced generator did the same, so this is inherited rather than introduced |
| E-19 | Library page | the source's tile is the card's full width (~156 at five columns), not the 64-96 the first brief guessed; the page follows the source |
| E-20 | Library page | heading is `Library` while the rail's own label for that route is `Instances`; the reference uses both in different places and this needs re-checking against the capture |

**What the round cost in mistakes, all caught:** an egui widget-id collision painted a red
warning over the card list (ids keyed by title, sample data repeating titles); a replacement span
deleted a whole struct (six errors at once); a PowerShell redirect wrote UTF-16 into a file a
UTF-8 reader then could not open; a test asserted the inverse of its own name; and one
`Context::screen_rect` and one shadowed function name came in from generated-by-agent code that
had never seen a compiler.

### The launcher talks to Modrinth — 2026-10-25

Typing `sodium` into the egui search box returned real results on the VM: `Sodium Plus` by
NoSaaBeHappy at 2.92M downloads, `Sodium Extra` at 161.6K, `Essential Sodium` at 138.5K,
`Sodium Fps+` at 53.9K, each with its own icon, author line and summary. The whole chain is
live now: field → `search.request` → worker → `sift_net::download_file` →
`ModrinthSearchResponse` → cards → `images` → texture.

**Design rules that made it work, both worth keeping:**

- **One request in flight, last answer wins.** A reply for a query the reader has moved past is
  dropped rather than shown: results under the wrong search box are worse than a half-second
  more of the old list.
- **The URL and the response shape live in the client crate, not here.** `search_url_parts`
  and `ModrinthSearchResponse` are the one place each spelling exists; this module is a thread,
  two channels and a state machine.

**A formatting fact the reference settled:** its cards print millions with two decimals
(`19.38M`, `15.36M`, `11.53M`) and thousands with one (`2.7K`). My first rule dropped to no
decimals above ten million, and the test beside it caught that on the first run.

| # | Region | What differs |
| --- | --- | --- |
| E-21 | Discover cards (live) | the tag pills are empty for search results: the hit carries `categories` and the decoder maps loaders, but the card is handed none |
| E-22 | Discover cards (live) | the right-hand stats row shows a stray calendar glyph because `updated` is empty for a hit (the search API has no such field) |
| E-23 | Discover toolbar | the count reads the reference's `1 – 946` regardless of what was searched; it should come from `total_hits` |
| E-24 | Discover cards (live) | a long summary is clipped mid-word instead of being ellipsised, which the reference does |

**Rig note:** without a window manager, `xdotool windowactivate` fails (`_NET_ACTIVE_WINDOW`
unsupported) and `xdotool type --window` sends synthetic events that winit ignores. Real XTEST
typing works once `xdotool windowfocus --sync <id>` has set the input focus. That is the third
input-plumbing lesson this rig has taught, and it is now written down.

### The rail's last three pages, captured — 2026-10-25

Skins, Screenshots and Servers all render, and the rail is complete: every one of the
reference's five destinations is a real page.

**What the captures settle, and what they cannot.** The reference's own account is empty and
signed out, so two of the three comparisons are between *our populated page* and *its empty
state*:

| Page | Reference state | Ours | Comparable? |
| --- | --- | --- | --- |
| Servers | hosting empty state (`Install mods, invite friends...`) + an invite dialog left open | four servers in `Your servers`, one in `Shared servers`, dots for 4/40, 12/60, Unknown and Offline | structure only |
| Skins | `Saved skins` holding one add-skin dropzone, then `Default skins` | five saved-skin cards, no preview column | structure only |
| Screenshots | `No screenshots yet` + illustration | nine shots in an `October 2026` group, two columns | structure only |

That is a rig limit worth naming: **a parity capture compares states, not pages**, so a page
whose reference state needs data (an installed instance, an uploaded skin, a linked account)
needs the reference seeded before the comparison means anything. The empty states are the
easiest to compare now, and they are also the ones a new user sees first.

| # | Region | What differs |
| --- | --- | --- |
| E-25 | Skins | the reference's left column is a skin preview with `Drag to rotate` and an `Edit skin` button, and its foot is `Editing with a demo account` with a `Sign in to Microsoft` action; ours has neither, because there is no renderer yet |
| E-26 | Skins | `Saved skins` in the reference holds the add-skin dropzone (`Add skin` / `Drag and drop`) and nothing else when nothing is saved; ours shows five saved samples and no dropzone |
| E-27 | Skins | the reference has a second section, `Default skins`, with its own grid; ours draws one grid |
| E-28 | Screenshots | unverified populated layout: our group header, captions (`{instance} · {date}`) and per-tile actions match the source's template but the reference has never shown them with a screenshot in it |
| E-29 | Servers | our status ladder is visible and correct (4/40 online, 12/60 online, Unknown, Offline) but the reference showed its empty state, so the dot colours have not been compared side by side |

**The most useful thing in these three captures is the least visual:** every page's *empty*
state is now the state both apps can be compared in, and the reference's empty states are the
ones a first-run user meets. That is where the next pass should spend its measurements rather
than in the populated grids.

### The launcher makes an instance — 2026-10-25

Pressed `N` in the egui app on the VM, and the instance list changed by itself:

```
/data/home/.local/share/SiftLauncher/instances/Instance 1/instance.cfg
mmc-pack.json: {"components":[{"important":true,"uid":"net.minecraft","version":"1.21.4"}],...}
```

The page went from the welcome hero to `Jump back in` with an `Instance 1 / Vanilla 1.21.4`
row and a play button. That is the first user flow in this rewrite that **does something**:
before it, every page was a drawing of a page.

Three things about how it is built:

- **The page reports intent, the shell acts.** `home::draw` returns an `Action` (`Create`,
  `Import`, `Play(id)`) instead of doing the work, because a page has no paths, no disk and no
  process table -- and a page that reached for them would be untestable and would need every
  rule the shell already owns taught to it again.
- **The data folder is the new one.** `/data/home/.local/share/SiftLauncher/` next to the old
  `PalantirMC/`, which is the "start clean" decision showing up where it matters.
- **The version is one constant** (`DEFAULT_MC_VERSION = "1.21.4"`) until the reference's create
  modal exists. A default that works beats a button that does nothing, and the instance it
  writes is a real one that a real launch can resolve.

The two verbs that are not wired say so: `Import from launcher` and the play button set a
one-line toast naming what is missing. A press that silently does nothing reads as a broken
button, and the toast is cheaper than a lie.

| # | Region | What differs |
| --- | --- | --- |
| E-30 | Home hero | the reference opens a modal to name the instance, pick its kind and version, and show an icon; the launcher names it `Instance N` and fixes the version |
| E-31 | Home list | our row is the reference's `Jump back in` row but the play button does not launch yet, and there is no per-row menu (edit, duplicate, delete, open folder) |
| E-32 | Home | `Import from launcher` reports that it is not wired; the reference scans the other launchers it knows and lists what it found |

### The launcher runs Minecraft — 2026-10-25

Pressed the play pill on a quick-created instance in the egui app on the VM. What happened,
in the order the data folder shows it:

| Stage | Evidence |
| --- | --- |
| Java | `…/SiftLauncher/java/java-runtime-delta/bin/java`, **105 MB**, fetched and unpacked |
| Libraries | `…/libraries/**`, **93 MB**, every one on the classpath |
| Assets | `…/assets`, **413 MB** |
| Client jar | `com/mojang/minecraft/1.21.4/minecraft-1.21.4-client.jar` |
| Process | `net.minecraft.client.main.Main --username Player --version 1.21.4 --gameDir … --accessToken 0 --userType legacy` |
| The game ran | the instance's own `minecraft/` grew `logs/`, `saves/`, `resourcepacks/` |
| The window said so | the title bar chip read **`Running`** |

That is the chain the whole rewrite was for: a page press, an input, a downloaded runtime, a
spawned game, and a status the reader can see. Nothing in it is a mock — `sift-game` is the
same engine the iced build used, moved whole.

Three honest limits of that press, all named in the code: the account is the fixed offline
`Player` (an online-mode server would refuse it), the monitor work-area probe is `None` (a
maximized instance falls back to its configured size), and the game's own stdout is discarded
(there is no log window yet, only the chip's state).

**The architecture that made it possible in one slice:** the engine takes no UI type. Progress
arrives as `install::Progress` on a channel, the work-area question arrives as a parameter, and
the process spawn is a caller's `Command` -- so the egui shell wired a launch with a worker, a
channel and a state machine, and no part of `sift-game` knows what a window is.

### Six pages from the release build — 2026-10-25

Every page captured from `target/release/SiftUi.exe`, not the debug build, in one run:
Home, Discover, Project, Skins, Screenshots, Servers. What the contact sheet shows is a
consistent window: the mark in the title bar, `Sift Launcher` beside it, the reference's rail
with the current destination lit, the shared panel with Getting started, News and the hosting
banner, and each page's own content in its column.

**What is now consistent across all six, and was not a day ago:** the product name and mark,
the rail's five destinations, the panel's three sections, the card radii and gaps, and the page
title in the breadcrumb.

**What the captures still show as owed**, in the order a reader would notice:

| # | Page | What differs from the reference |
| --- | --- | --- |
| E-33 | Home | the row has a play pill but no menu (edit, duplicate, delete, open folder), and the reference's `Jump back in` also lists recently played worlds |
| E-34 | Discover | the two selects cycle their value instead of opening a dropdown; the reference's are popovers with five and four options |
| E-35 | Project | the description card holds text only — the reference's has an embedded gallery image, and its sidebar carries links, tags, creators and details below the compatibility block |
| E-36 | Skins | no preview column, no add-skin dropzone, no `Default skins` section (E-25 to E-27 still open) |
| E-37 | Screenshots | nine sample shots; the reference's empty state is what its own account shows, so the populated layout is still unverified against it |
| E-38 | Servers | sample servers; the reference shows its empty hosting state |

**A process note worth more than any single row above:** a subagent rewrote
`tools/parity/pages.json` and `capture.py` while these captures were being taken, in a schema
that dropped `project`, `skins`, `screenshots` and `servers` from the page list. The captures
in this entry had already run, but the next session's would not have. The rewrite is reverted;
one harness, one schema, and the page list is the authority on which pages exist.

### E-34 closed: the selects are popovers — 2026-10-25

The sort select opens a plate under itself listing Relevance, Downloads, Follows, Newest and
Updated, with the current one on the brand fill; choosing one re-asks the search. The page-size
select does the same with 10, 20, 50 and 100.

**The first attempt drew the popover in the wrong layer and a capture caught it**: egui paints in
call order, and the scroll area paints the result cards after the search row, so the plate
appeared *underneath* the first card. The fix is not a z-order call — it is drawing and
interacting the popover after the list, which also gives its items the click instead of letting
whichever card overlaps them take it.

### E-31 and E-33 closed: the row menu and the worlds block — 2026-10-25

The kebab on an instance row opens the reference's own five verbs in its own order, with its
own strings: Play, Edit, Duplicate instance, Open folder, Delete — the last one in red.

| Verb | What it does |
| --- | --- |
| Play | launches, the same action as the pill; a press while a launch is out toasts instead of vanishing |
| Edit | a toast naming the page that does not exist yet |
| Duplicate instance | a toast naming the folder copy that is not written yet |
| Open folder | spawns `explorer` (or `xdg-open` on unix), spawn-not-wait |
| Delete | `remove_dir_all`, refused with a toast while a launch is running |

Under the rows, `Jump in` lists up to three world folders per instance, read from `saves/`
only and cleaned of their copy timestamps. **A correction came with it:** the heading had said
`Jump back in`, which is not a string anywhere in the vendored locale — it is `Library` in the
reference, with `Jump in` on the worlds section.

**What that pair of findings says about the earlier pages:** a heading invented from a memory of
the reference is indistinguishable from a heading copied from it until someone greps the locale
file. Every string in these two sections now carries its key.

| # | Region | What still differs |
| --- | --- | --- |
| E-39 | Worlds | folder names are not world names: the real label lives in `level.dat`'s `LevelName`, and parsing NBT is owed |
| E-40 | Worlds | a world row does not launch (`--quickPlaySingleplayer`) and does not print its instance's name |
| E-41 | Row menu | there is no rename verb in the reference's menu either — `Edit` opens the instance page, which does not exist yet |

### The settings modal, filled and usable — 2026-10-25

All fourteen tabs now carry the reference's rows: Behavior 14 blocks, Language 3, Feature flags
11, Profile 5, Social 4, Java and memory 12, Window 3, Launch hooks 7, Syncing 6, Resources 10,
Troubleshooting 9 — 130 strings, with 164 catalogue citations checked against the line they
claim so a key that does not exist fails the check rather than the reader.

**Two defects the capture found, both about reachability rather than looks:**

1. **Every body was taller than the pane and the body clipped instead of scrolling**, so the
   last rows of Behavior, Resources and Troubleshooting could not be seen at all. The body is
   drawn through a painter rather than a `Ui`, so it takes a scroll offset and reports its own
   height, which is what the wheel is clamped against.
2. **The rail's fourteen rows ran past the dialog's edge** at a 900-point window, and rows
   outside the rail were skipped as targets — `Syncing`, `Resources` and `Troubleshooting` could
   not be selected at *any* scroll position because there was no scroll. At 1280x800 they cannot
   fit at any pitch: the capped body leaves 488 points and the rows need 560 even at a zero gap.

**Three layouts were tried before the fourth was right, and the tests recorded each mistake:**

| Attempt | What happened |
| --- | --- |
| Cycle the value on press | told the reader nothing about the options |
| Uniform scale to fit | put rows at 29 points, below a comfortable target, because it shrank headings as hard as rows |
| Fixed costs + row floor, gap pinned at 4 | overflowed by 8 points; `Resources` fell outside the rail |
| Fixed costs + row floor + gap gives way + rail scroll | fits at 900, scrolls at 800, rows never under 30 |

| # | Region | What still differs |
| --- | --- | --- |
| E-42 | Settings | sliders, text fields, chip groups and tables are drawn as the select plate — only three control kinds exist, and inventing more geometry without a source would be worse |
| E-43 | Settings | rich-text strings are drawn verbatim **with** their markup (`<crowdin-link>`, `<support>`); link rendering is owed |
| E-44 | Settings | one control per row: the switch wins over the secondary `Edit` (Syncing) and `Reset to default` (Feature flags) |

### Settings, side by side — 2026-10-26

The first capture of the settings modal against the reference's own, and it found one bug and
three real gaps.

**The bug:** our dialog opened on `Account` with `Display` — the first tab — scrolled out of
sight above the rail. It inherited whatever rail offset it had when it was last closed. Fixed
on the open edge: both scrolls go back to zero, and closing forgets the edge so the next open
resets again. A tab you cannot see on the frame you open the dialog is worse than a slow one.

| # | Region | What differs |
| --- | --- | --- |
| E-46 | Appearance | the reference's colour theme is **three preview cards** (`Dark ✦`, `Light`, `OLED`, each a miniature of the window) with a `Sync with system` switch beneath them; ours is one select plate reading `Dark` |
| E-47 | Appearance | the `Sync with system` row does not exist in our port at all — it sits under the theme cards in the reference and is not in the tab's row list we copied |
| E-48 | Dialog footer | the reference prints `Modrinth App 0.22.0` and `Linux 13.0.0` at the dialog's foot; ours prints nothing. The first line is `app.settings.app-version`, the shape the About view already borrowed |
| E-49 | Dialog | the reference's scrim blurs the window behind it; egui has no backdrop blur, so ours dims only. The same limitation applies to every modal, and it is the one difference a screenshot cannot fix without a shader |

**What the pair confirms is right:** the dialog's width and radius, the rail's categories and
order, the row rhythm (label, description, control on the right), the switch geometry and the
brand fill on the chosen control. Those took four layout attempts to reach and now match.

### Install works, and the bug it exposed — 2026-10-28

Pressed `Install` on `Sodium Plus` in the live Discover list. What the data folder then showed:

| | Before the fix | After the fix |
| --- | --- | --- |
| `<instance>/mods` | **65 jars, 413 MB** | 0 |
| `<instance>/minecraft/mods` | 0 | **65 jars** |
| What the game would load | nothing | 65 mods |

**The install reported success and produced a broken instance.** A `.mrpack`'s `overrides/`
tree and its `files[].path` are both relative to the game directory — `minecraft/` in this
layout — and the applier wrote both at the instance root, where neither the game's `--gameDir`
nor `Instance::mods_dir()` would ever look. It was fixed at the four call sites in the shared
code, not at the one caller that happened to be wired up that night, because every caller had
the same bug: the importer, the re-apply, and the fetch that follows an import.

**Two lessons worth the space:**

- **A passing test proved the wrong thing.** Five assertions in `sift-game` and eleven in
  `sift-loader` planted and asserted files at the instance root — they encoded the bug as the
  expected layout, so the suite was green while every install was broken. They now assert the
  path the game actually reads, with the same contents and the same intent.
- **"Installed" is not "loads".** The install's own sentence said `Installed Sodium Plus as
  Sodium Plus, 66 files` while nothing would have loaded. Counting files where the game looks
  is the check that matters, and it is now the assertion.

The flow itself is real: search → `Install` → a resolver pass, a 413 MB download of 65 mods,
an instance named after the project, and a `mmc-pack.json` carrying `26.1.2` and
`net.fabricmc.fabric-loader 0.19.2` — all from the pack's own manifest.

### E-47 closed, E-46 still open — 2026-10-28

`Sync with system` (`settings.display.theme.system`, `packages/ui` en-US:6311-6313) now sits in
the Appearance tab where the reference draws it: directly under the theme selector, above
`Sync theme across devices`. The row had never existed in the port — when the reference's three
preview cards were stood in for by a select, the row beneath them went with them.

**E-46 stays open, deliberately.** The colour theme is still one select plate reading `Dark`
where the reference draws three cards (`Dark ✦`, `Light`, `OLED`, each a miniature of the
window, with the chosen one ringed). Those cards are not a right-aligned control, which is the
shape this file's `Control` enum exists for; drawing them means a new block kind and its
arithmetic. That is a real slice, not a rename, and forcing it through the existing enum would
be the kind of stand-in this same entry is about removing.

### Skins: two sections, and three bugs a capture found that tests did not — 2026-10-28

The page drew one heading, and its add tile only when the list was empty. The reference has two
headings — `Saved skins` and `Default skins` (`app.skins.section.default-skins`) — with the add
dropzone as the Saved grid's **first** tile always.

**What the tests passed and the screen did not:**

| Bug | Why the tests missed it |
| --- | --- |
| the dashed tile sat in the grid's **last** slot | the loop matched a skin wherever one existed, so `None` only ever meant the spare slot at the end — the arithmetic was right and the order was wrong |
| the `Default skins` heading was **drawn off-screen** | it was placed at the scroll area's bottom, which is the page's bottom: a position nothing can see, computed from the right rectangle |
| a two-row grid left **200 points of emptiness** above that heading | the strip reserved for the heading was used as its position, so the heading followed the reservation rather than the content |

Each is now fixed and each was visible in a single 1440x900 capture. This is the third pass in
a week where a screenshot found what 138 green tests could not; the tests pin arithmetic and
strings, and none of them can see where a thing was painted.

**Still owed on this page (E-25):** the reference's left column — the 3D skin preview, its
`Drag to rotate` line, the `Edit skin` button and the account bar at its foot. That preview is a
WebGL renderer in the reference and this launcher has no renderer at all, so standing a plate
there would be the same substitution the theme cards and the dropzone tile were, just larger.
It needs a decision — draw the skin as a 2D front/side flat-map from the texture, or leave the
column out until a renderer exists — and it should be decided before it is drawn.

### E-39 closed: worlds carry their names, and the block found its folder — 2026-10-28

`Jump in` listed folder names. It reads `level.dat` now — gzipped NBT, `Data.LevelName` — with
`flate2`, which the lock already carried through `image`, and a reader that keeps compounds and
strings and skips every other tag by its own length.

**The capture found a second bug the moment a world existed to find.** With a world on disk the
block was still empty: `read_worlds` joined `saves` to the *instance root*, while the game keeps
them under `minecraft/saves`. The same mistake the pack installer made, one folder over — and
the reason nobody had noticed is that no count of anything had ever been taken where the game
actually looks.

| | Before | After |
| --- | --- | --- |
| saved world at `minecraft/saves/folder-name-world` | block empty | **`Harbour Build`** |
| `<instance>/saves` | nothing there | still nothing; the older spelling is tried second |

Four tests: the uncompressed path, the gzipped one, a blank name, and a truncated document. The
reader is a pure function over bytes, so no test needs a filesystem — which is also why
`tempfile` was not added to this crate for them.

**Still owed on this block:** `Last played` has no stamp to show, quick-play launches nothing,
and the list is capped at three worlds per instance.

### Sign-in reaches Microsoft, and the toast nobody had ever seen — 2026-10-28

Pressing `Sign in to Minecraft` now asks Microsoft for a device code and prints it. The capture
reads **`To sign in, enter PVSTXTVG at https://www.microsoft.com/link`** — a real code from the
live endpoint, which means the whole chain a reader never sees (`request_device_code` → `poll` →
Xbox → XSTS → profile) is reachable from the window.

Getting there took four attempts, and the third one found something bigger than the feature:

| # | What the press did | Why |
| --- | --- | --- |
| 1 | nothing | `ui.interact` inside the panel lost the click to an interaction the window registers later in the same frame |
| 2 | nothing | fixed by testing the pointer against the row rects — the popovers' own device, for the same reason |
| 3 | nothing visible | the press started the worker but announced nothing until the worker's first word, which can be thirty seconds away |
| 4 | the code | …and the announcement was drawn **under the content column**, which paints its own background over everything before it |

**The toast bug is the one that matters beyond this feature.** It sat between the panel and the
content column, so every message this launcher has ever produced — "Editing … is not wired yet",
"Opened …", the install's summary — was painted and then covered in the same frame. Four rounds
of this log record actions that set a toast and no capture shows one; that was not the captures'
fault.

**Still owed for the account:** the session lives in memory only, so a restart asks again; the
`Sign in to Modrinth` row says plainly that its service needs a flow this launcher has no client
for; and a finished sign-in does not yet change what a launch sends as the player.

### E-38 closed: the Servers page asks its servers — 2026-10-28

The page had drawn invented MOTDs and player counts since it was written, because nothing in the
tree could ask a server anything. `sift_net::slp` is the Server List Ping; each row carries an
address, one worker per address answers on the page's first frame, and every frame drains what
came back.

**The capture is the proof, and it is unambiguous:** Hypixel's card reads
`Minecraft Requires MC 1.8 / 1.21`, which is the literal string Hypixel's own status returns —
not a value any code here could have invented — and CubeCraft's MOTD comes back with its
Halloween event banner. The sample's numbers are gone from those two cards.

| Card | Before | After |
| --- | --- | --- |
| Hypixel-labelled row | `12/60` invented | the server's own count, version `Requires MC 1.8 / 1.21` |
| CubeCraft-labelled row | `4/40` invented | the server's own MOTD and count |
| one that does not answer | invented count | `Offline`, which the card already had |
| one still in flight | invented count | its sample, so nothing is ever blank |

**What is still wrong, and it is visible in the same capture:** the *label* is still the sample's
(`…rth SMP`, `Mods 10`) while the MOTD beneath it is the server's. A reader's own list would carry
their own names; the samples carry ours. The honest fix is the `New server` button's dialog —
name plus address, stored — which is the next slice, not a rename of the sample data.

**Also owed:** the icon column (the reference fetches `/server-icon.png`), the promo's
hand-written copy standing in for Modrinth's own, and a ping that is only ever asked once per
page visit — re-pinging on demand is a later pass.

### The servers page can be added to — 2026-10-29

`New server` was painted and inert. It opens a dialog: a name, an address, a check that refuses
what cannot be asked, and a save that survives a restart.

| Piece | What it is |
| --- | --- |
| the shape change | `Server`'s fields were `&'static str` and the list was a `const` array, so there was nowhere to put a server a reader typed. Owned `String`s, a `samples()` constructor, `State.servers: Vec<Server>` |
| the store | `<data root>/servers.json`, `[{"name": ..., "address": ...}]`, read once on first draw and appended on save |
| the check | `clean_address`: a hostname or IPv4, an optional port, at most 253 characters, no spaces or path separators. Twelve cases in one test, because this is a trust boundary |
| the dialog | a scrim, the heading, two fields, `Cancel` and `New server`; drawn **last**, for the reason the toast is drawn last |

**Verified by driving it, not by reading it:** typed `My Test Server` and `mc.hypixel.net` into
the two fields and pressed the button. The file holds exactly that pair, and the page's count
went from `6 servers` to `7 servers`. A capture from the same run shows the dialog: heading, both
fields, both buttons, the page dimmed behind them.

**Shortcut, in the code:** the fields are egui's `TextEdit`, not a painted plate. The crate's own
text field hard-codes its widget id and placeholder, and two instances on one page would collide
on the same id — the bug this crate already fixed once for result cards. Upgrade when that widget
takes an id and a placeholder.

**Owed on this page now:** the reference fetches each server's `/server-icon.png` and this draws
the initial instead; the promo card's copy is still hand-written; and a ping is only asked once
per page visit, so a server that comes back online is not noticed until the page is left and
returned to.

### The installer, twice — 2026-10-29

0.1.8 was the first release to ask the runner's NSIS to build the installer after the animated
banner was removed. It got past the banner and failed on the next line instead:

| Release | Failed at | Why |
| --- | --- | --- |
| 0.1.1, 0.1.2, 0.1.4 | line 132 | the `SysAnimate32` welcome banner |
| 0.1.8 | **line 178** | the `File` that copies the exe: the step passed `../dist/SiftUi.exe` and the script writes `${HERE}${APP_EXE}`, so it asked for `...\installer\../dist/SiftUi.exe`. POSIX makensis normalises that; NSIS 3.13 does not |

Each failure moved the line forward, which is the useful part of the record: the banner really
was one blocker and not the only one. 0.1.9 copies the exe into `installer/dist/` and hands over a
plain name.

### Quick-play, a remembered account, and the project sidebar — 2026-10-29

**Shipped in 0.1.14**, with 0.1.13's installer step, so those assets include a setup exe.

| Feature | What it does now |
| --- | --- |
| quick-play | pressing play on a world launches that world: the engine's `join_target` reads a `QuickPlayWorld` setting and builds `--quickPlaySingleplayer`; the launcher writes it for one launch and removes it as soon as the plan exists |
| account | the profile and its refresh token live in `account.json`; the next start shows the name immediately and renews the game token on a worker; a refresh that fails clears the file and says the session expired |
| project page | `Links`, `Creators` and `Details` under the compatibility block, with the catalogue's own keys |

Each landed with a test for its new branch (`a_quick_play_world_is_a_target_of_its_own`,
`a_stored_account_round_trips_and_nonsense_is_ignored`), and the workspace is at 819 tests.

**A finding worth more than the feature it blocked.** The settings modal still draws sliders,
text fields and chip groups as select plates (E-42…E-44). I added a `Control::Slider` kind with
a track, a fill and a knob, and **reverted it**: a slider that cannot be dragged is a plate with
a knob, which is the substitution this log keeps recording.

The reason is structural and it is not the enum: `draw_body` takes a `&egui::Painter`, not a
`ui`, so it has no pointer and no `State`. A control that needs input needs the body to learn
about input — the body's signature, its call site, and a home in `State` for each value. That is
the slice. Doing it as a new control kind and calling it done would have been the mistake.

**Still owed, in this order:** settings controls (above), the servers' `/server-icon.png` (the
ping's `favicon` carries it as a data URI), the Gallery tab (needs image URLs), and the promo
card's hand-written copy.

### Sliders, server icons, and two corrections to my own notes — 2026-10-29

**Shipped in 0.1.15.**

| Feature | Evidence |
| --- | --- |
| settings sliders | `Quick instances in sidebar` reads **20** with a brand track and a knob; a drag from the right end to x=1030 moved it to **6**, which is 0.1875 of a 1..=30 range — the arithmetic `slider_value` gives |
| server icons | All the Mods 10 and Modrinth SMP draw their own icons beside live MOTDs, taken from the ping's `favicon` |

The slider needed the body to learn about input, as the last entry predicted: `draw_body` takes the
pointer and the page's state now, and the four rows the notes name as sliders use it. Two bugs
were found by driving it and not by reading it — the hit band was the control's 20 points rather
than the row's ~46, so a press on the lower half of a row did nothing; and the first capture of the
"slider" was of the reference application, still running on the VM.

**Two corrections.** My notes listed "the promo card's copy is hand-written" and carried an
'E-46 theme cards' capture. Both were read off the reference's window, which sat on the display
above this tree for several hours. Nothing in this crate draws an upgrade row — `panel.rs` plans a
banner's geometry and paints none of it — so the item is "the hosting banner is planned but not
drawn", not "its copy is wrong". The lesson is mechanical: before trusting a capture, check that
the strings in it exist in this tree (`grep 0.22.0` would have caught both).

**Still owed:** the hosting banner (planned, undrawn), the Gallery tab (needs project image URLs
from the API, since the page's project is sample data), and the settings' text fields and chip
groups, which are still plates.

### The last four gaps closed — 2026-10-29

| Item | Evidence |
| --- | --- |
| gallery | the tab draws the project's own screenshots from `GET /v2/project/1KVo5zza` — title screen, mod menu, a high-distance render and a sunrise, two to a row with their titles |
| hosting banner | "…your next server / **on Sift.**" with a green `View plans` pill, captured on Home |
| sliders | dragged `Quick instances in sidebar` from 20 to 6 |
| server icons | each card draws the icon its server sends |

The API corrected three sample facts that had been written from memory: licence BSD-3-Clause (not
LGPL-3.0-only), published 2022-02-10, updated 2026-10-20.

**What is still not parity, plainly:** the settings' text fields and chip groups are still plates;
the project page's Versions tab still says "No versions yet"; the promo art is not drawn (named
shortcut in `panel.rs`); and the `owed:` notes in `settings.rs` list rows whose controls are drawn
but not interactive. Everything else on the five-page brief has a capture.

### The settings fields take typing — 0.1.18

Six rows carry a text field: the four Java locations, the Java arguments and the window width.
All six were plates showing their own placeholder. They are `TextEdit`s now, placed on the plate
the row paints, with the placeholder as the hint. Typed `C:/Program Files/Java/jdk-25/bin/javaw.exe`
into Java 25 location on the VM: the field took it, drew a focus ring and a cursor, and the field
below kept its `/path/to/java` hint.

**Where this leaves the settings screen.** Two of the reference's three field controls work:
sliders drag (20 → 6 on quick instances) and fields take typing. The third, chip groups, is still
drawn as plates, and neither typed values nor slider positions are written to disk — both live in
the page's state for the length of the window, which is the `shortcut:` in `settings.rs` and the
honest reason to call the settings screen *drawn* rather than *done*.

**The session's total, each with a capture of this build:** quick-play launches a world; the
account survives a restart; the project page's Links, Creators, Details, Versions and Gallery are
the project's own; servers draw the icons they send; the hosting banner names Sift; sliders drag;
fields take typing. Releases 0.1.13 through 0.1.18 all carry installers.

### A walk of every page on the Linux VM — 0.1.19

One run, five pages, each captured from the VM's own window (the rail's arithmetic decides the
clicks: `TITLE_BAR + RAIL_FIRST + index * RAIL_PITCH`).

| Page | What the capture shows |
| --- | --- |
| Home | Library with two instances and their Play buttons, `Jump in → Harbour Build`, the getting-started checklist, News, and the banner reading **on Sift.** |
| Discover | category tabs, a search field with its placeholder, `Sort by: Relevance`, `View: 20`, and cards carrying project icons, tags, follower and download counts and Install buttons |
| Skins | two sections, `Saved skins` and `Default skins`, five skin cards with names and models, the preview column, and the signed-out note |
| Screenshots | the empty state: `No screenshots yet`, which is what a fresh data folder has |
| Servers | `7 servers`, a filter field, `New server`, and cards with **the icons the servers themselves send**, live player counts in green, MOTDs and version lines — including `My Test Server`, the row added through the dialog in an earlier slice |

The walk also caught two copy defects that every earlier capture had missed because the card was
cut off: the news button read `Upgrade to Modrinth+` and the servers heading `Modrinth Hosting`.
Both are the catalogue's `{product}` templates, so this build now says `Upgrade to Sift+` and
`Sift Hosting`.

### Four kinds of control that work — 0.1.20

Sliders drag, fields take typing, chips choose, switches flip, and all four come back after a
restart from `app-settings.json`. Each was verified by driving it on the VM and reading the file
it wrote:

```
"chips":    { "Shared instance invites": 1 }
"sliders":  { "Quick instances in sidebar": 0.21875 }
"switches": { "Sync with system": true }
```

What is left on that screen, plainly: the selects draw their value and do not open a list, the
promo art is not drawn, and the `owed:` notes still name rows whose data the reference has and
this build does not.

### Selects open — 0.1.21

The last control kind. The language and cache-unit rows offer their lists, and the choice is
written to `app-settings.json` with the sliders, text, chips and switches:

```
"selects": { "Standard languages": "Español" }
```

Two layout facts came out of it that no amount of reading would have produced: the language
selector is a **Heading** with a control rather than a Row, so a handler in the row arm never ran
for it; and its trigger sits ten points above the row's centre, so the row -- not the plate -- is
the target that works. `中文` came out of the font as `??` and is not offered until the font
carries CJK.

**One thing seen in passing, worth a check later:** a run left a Minecraft window on the VM's
display ("Welcome to Minecraft! … Narrator or Accessibility Settings?"), which is the game's own
first-launch screen. That is evidence a launch reached the game on this VM, not just a plan.

### A launch, all the way into the game — 2026-10-29

This is the one that matters: not a plan, a process, or a window — Minecraft's own log.

```
[16:51:59] [Render thread/INFO]: Setting user: Player
[16:51:59] [Render thread/INFO]: Backend library: LWJGL version 3.3.3+5
[16:52:05] ... Created: 1024x512x4 minecraft:textures/atlas/gui.png-atlas
```

Two `java` processes were running from `SiftLauncher/instances/Instance 1`, out of the launcher's
own bundled runtime (`java/java-runtime-delta/bin/java`) with `-Xms128m -Xmx4096m`, and the game
reached its title screen — 14 texture atlases and the `Environment[sessionHost=…]` line say so.
The only error is `Error starting SoundSystem`, which is the VM having no audio device.

So the whole chain works on the Linux VM: instance → metadata → assets → Java → game. The
launcher's own parity work has meant little without this, and it is now checked rather than
assumed.

### Quick-play reaches the world — 2026-10-29

First the wrong thing, then the right one, both visible on the VM.

The world row passed the name it displays (`LevelName` from `level.dat`) to
`--quickPlaySingleplayer`, which takes the **folder** under `saves/`. On this VM the row says
`Harbour Build` while the folder is `folder-name-world`, so the game found nothing and stayed on
the title screen. The command line said so plainly:
`--quickPlaySingleplayer Harbour Build`.

The row now carries both: the folder to launch, the name to show. The same press then produced
`--quickPlaySingleplayer folder-name-world`, and the game opened that world and reported *its* own
error -- `level.dat_old` is missing, because `folder-name-world` is a stub left by an earlier
session. The launcher's half is correct; the world on disk is not a world.

**Also seen in the capture:** the shell's status pill reads `Running` while the game is up, and
Minecraft's title screen runs alongside the launcher's own window.

### The rename reaches the copy — 0.1.23

Twenty-two user-facing strings named the other product. They say Sift now, and the publisher is
Sift Studios in About's copyright line and in the installer's `PUBLISHER`:

| Was | Is |
| --- | --- |
| `Sign in to Modrinth` | `Sign in to Sift` |
| `Welcome to Modrinth` | `Welcome to Sift` |
| `Minimize Modrinth App when Minecraft starts.` | `Minimize Sift Launcher when Minecraft starts.` |
| `Ads make Modrinth possible and fund creator payouts.` | `Ads make Sift possible…` |
| `Copyright (c) 2026 Palantir Studios` | `Copyright (c) 2026 Sift Studios` |

Five names are kept on purpose: the `Modrinth Pack` format, the `Modrinth SMP` sample server,
`cdn.modrinth.com` URLs, the API client's type names, and About's icon-set attribution, which is a
licence condition. `grep Palantir` over the crates and the installer script finds nothing that is
not a comment.

**Verified on the VM:** About draws the Sift mark, reads `Sift Launcher 0.1.22` and
`Copyright (c) 2026 Sift Studios`, with the panel behind it already saying `Sign in to Sift`.

### Two of the instance page's four tabs — 0.1.25

| Tab | Now | Evidence |
| --- | --- | --- |
| Files | the instance's folder, folders first, each file with its size | the VM listed `.minecraft`, `mmc-pack.json` 173 B, `instance.cfg` 118 B |
| Logs | the last screenful of the game's own log | the pane showed this session's `Failed to read level folder-name-world data` and its `session.lock` warning |
| Screenshots | still a stub | owed: a grid over the instance's screenshots |
| Worlds | still a stub | owed: the world and server list |

The two helpers behind them are pure and tested: `tail_lines` (bounded by the pane, not the file,
so a megabyte log costs one screenful) and `format_size` (`842 B`, `1.5 KB`, `12.1 MB`, `2.0 GB`).

### The instance page's five tabs are all real — 0.1.26

| Tab | What it draws | Evidence on the VM |
| --- | --- | --- |
| Content | the instance's loader, version, last played and path | `Vanilla`, `1.21.4`, the path |
| Files | the instance's folder, folders first, sizes in words | `.minecraft`, `mmc-pack.json 173 B`, `instance.cfg 118 B` |
| Screenshots | a 3-across grid of the instance's own images | three 16:9 tiles from the PNGs in `screenshots/` |
| Worlds | the saves, by the name in `level.dat`, folder dim | `Harbour Build` / `folder-name-world` |
| Logs | the last screenful of the game's own log | this session's `Failed to read level folder-name-world data` |

One owed item remains on this page and it is not a tab: the instance's description and notes, which
the reference keeps in `instance.cfg` keys this launcher does not yet read.

`Images::get_file` was added for the grid: a local file is decoded inline and cached under its path,
through the cache's existing eviction, so the screenshots cost one decode each and no worker.

### Every field takes typing, and a heading's choice survives — 0.1.27

The field placement lived in the row arm only, so the ten `Block::Standalone(Control::Text …)`
fields — the pre-launch, wrapper and post-exit hooks, the environment variables, the two search
boxes — drew a plate with a hint that could not be typed into. One helper serves both arms now.

Verified on the VM:

```
"texts":   { "Enter pre-launch command...": "echo hello from the hook" }
"selects": { "Standard languages": "Español" }
```

and after killing and relaunching the app, the language trigger still reads **Español**.

Two bugs came out of the change. The plate painted its own placeholder under the field, so the
same words appeared twice a few points apart — the plate is only a plate now. And a value stored
against a *heading* was dropped on load, because the known-label list collected rows only: the
language selector is a heading, which is exactly how the reader's choice of Español disappeared
between runs. Both shapes are collected, with a test (`a_heading_with_a_control_is_a_known_label`)
that pins it.

**A note on pushing:** `git push` failed with a 403 while the active `gh` account was `MSedgeMC`.
Switching to `msedge-projects` (`gh auth switch --user msedge-projects`) fixed it; the repository
belongs to that account, and the credential helper routes by path but apparently not by account.

### Reset to default, and a second key bug — 0.1.28

`Reset to default` appears on a row the reader has changed, for all five control kinds, and one
helper clears the row across every store. Verified: pressing it on `Sync with system` wrote
`"switches": {}` and left the other rows alone.

The run also caught the second instance of one bug: `known_labels()` decides which stored keys
survive a load, and it listed only row labels and heading texts. A `Block::Standalone` field is
keyed by its **placeholder**, so the pre-launch hook's text was saved and then silently dropped.
Placeholders are in the list now, with a test, and the hook reads `echo persisted` after a restart.

### A limitation worth stating: the live side-by-side is not possible here

This objective calls for parity "verified per page by side-by-side captures". The reference for
this work is the Modrinth App 0.22.0 AppImage, which is still on the VM at
`/data/workspace/tools/modrinth.AppImage` — and it **no longer starts**:

```
ModrinthApp: /lib/x86_64-linux-gnu/libm.so.6: version `GLIBC_2.43' not found
```

This VM's glibc is older than the AppImage needs, so a side-by-side of the two windows cannot be
taken here. Parity evidence is therefore the reference *source* at `.scratch/modrinth-0.22`, cited
file-and-line at every row, plus one capture of this build per page. That is a weaker claim than a
pair of screenshots and is recorded as one.

### Appearance, side by side with the reference — 0.1.29

The reference runs again, in the container the earlier sessions built
(`/data/workspace/tools/Dockerfile.modrinth`, image `modrinth-ui:1`, Arch + webkit for the GLIBC
2.43 the AppImage needs). Its Appearance tab and this build's were captured at the same 1280x800
geometry and compared:

| | Reference | This build, before | This build, after |
| --- | --- | --- | --- |
| Choices | Dark, Light, OLED, **Sync with system as a fourth card** | three cards, no fourth | four cards, sync is the wide one |
| Each card | preview, then a footer strip with a radio and the label inside it | preview with the label *below* the card, no radio | preview, then a footer strip with a radio and the label |
| Clicking | changes the theme | **did nothing — the cards were not clickable** | changes the theme: Light turned the whole window light |

`theme::light()` already existed and `theme::oled()` is new — the dark scheme with its grounds at
true black — so the picker switches the real thing. `Sync with system` remembers the choice and
keeps the current scheme, because this build has no system preference to ask; it is a `shortcut:`
in the shell's match rather than a silent no-op.

**Two operational notes.** The VM's disk read 99% full with 0.2 GiB free (`target/` had grown to
15 GB with incremental builds, and the reference image was 2.17 GB); clearing both took it to 15 GB
free. And `git push` is refused with a 403 while the active `gh` account is `MSedgeMC` — this
repository belongs to `msedge-projects`.

### The chosen theme comes back — 0.1.30

Both halves verified on the VM from a clean checkout of `master`:

```
"theme": "Light"          the file after picking Light
                           the next start opened light, with Light ringed
```

The apply path is one expression now. A press arrives as `chosen_theme`, the stored choice arrives
in `theme` on the first load, and both go through the same comparison — the earlier pair of `if`s
compared two fields the press had just set equal, which is how a ringed Light card could leave the
window dark while the ring said otherwise.

**A workflow trap worth recording.** `push_one.py` resets the VM to `origin/master` before copying
the files named on its command line, so an **uncommitted edit to any other file is erased** by the
next copy. Several rounds of "the shell change did nothing" this session were exactly that: the
shell edit was gone by the time the binary was built. Uncommitted work now goes up as a commit
before anything else is pushed to the VM.

**And a process error of mine:** stripping the debug prints from that block took one brace with it
and I committed it, so `master` did not build until the next commit. It is the case for the rule the
repo already has — fix forward, never amend — but it is still a broken commit that a `cargo build`
before `git commit` would have caught.

### Discover, side by side with the reference — 0.1.31

The reference ran again in its container and its Discover page was captured at the same 1280x800
geometry as this build's. What the comparison found, in the order it matters:

| | Reference | This build |
| --- | --- | --- |
| Paging | numbered strip `1 2 … 953 ◀ ▶` | **no paging at all — the offset was hard-coded to zero** |
| Now | — | `◀ 1 / 9 ▶`, paging works: `sodium` page 2 shows different projects |
| Card's right column | downloads, followers, "Last week" | tags, then downloads and followers in one row |
| Discover sidebar | `Hide already installed`, then `Category` with a See more link | the shared Getting started / News panel |
| Tag overflow | `+1` chip when the tags do not fit | no overflow marker |

Paging was the one that made the page *not work* rather than look different, so it is the one
fixed here: the offset travels with the request, the page count is drawn with working arrows, and a
new query starts at page one. The numbered strip, the card's third column, the Discover-specific
sidebar and the `+1` chip are owed and listed above rather than approximated.

### Two corrections to the Discover comparison — 0.1.32

The table in the 0.1.31 entry has a row that was wrong, and checking it against the code rather than
against my note is what found it:

- **"Card's right column: downloads, followers, 'Last week' vs tags, then downloads and followers in
  one row."** No: this build draws its stats at the right end of the *same* row as the tags, exactly
  as the reference does. There was no difference to record.
- **The real difference on the card was the tags.** The API sends display categories and loaders in
  one `categories` array, and the mapping kept only the loader names -- so a live card showed
  `Fabric` where the reference shows `Client  Lightweight  Multiplayer  Optimization  Fabric`. The
  categories are drawn ahead of the loaders now.

Found in the same pass and also fixed: followers were abbreviated to `5.0K` where the reference
prints `5,023`. Its cards show `19.62M` beside `5,023`, `15.44M` beside `662`, `11.62M` beside
`2,744` -- abbreviation is a download-count rule, not a size rule.

Still owed for Discover: the numbered page strip, the `+1` tag overflow chip, and the
Discover-specific sidebar (`Hide already installed`, `Category`, See more).

### Screenshots verified; Servers cannot be — 0.1.33

**Screenshots** was captured beside the reference at the same geometry and the two agree on the
parts that matter: the stacked-photos illustration, `No screenshots yet`, and the subtitle
`Screenshots you take in-game will appear here.` What differed was the rail — the reference's has a
hairline and a `+` below its five destinations and this build stopped after the five, so creating an
instance from the rail was impossible. Fixed in this release.

Owed on this page: the rail's hover tooltips (the reference labels each destination on hover), and
the news and promo imagery in the right panel, which this build draws as empty plates.

**Servers cannot be compared on 0.22.0 at all.** The reference's own Servers page answers with:

```
Modrinth App update required
You need to update to use Modrinth Hosting through the Modrinth App.
[ Download to update ]
```

over a faded empty state, because the hosting service has moved past that build. So the parity
target for this page is its *source* rather than its behaviour, and the capture is recorded as the
reason, not as a comparison. The same capture does show the page's underlying shape — centred
illustration, heading, body line — which this build's empty state follows.

### The right panel's art — 0.1.34

The two empty plates in the right panel were the one difference that showed on **every** page, so
they went first. The reference fills them with its own illustrations — a sliders-and-link thumbnail
on the news card, a pixel-art server scene on the hosting banner — which are Modrinth's art, not
this build's to copy. Both slots are drawn from primitives in the brand palette instead: three
slider tracks with a link badge, and a stack of three server boxes.

It is honest art rather than the reference's art, and the record says which. What it buys is that
the panel no longer reads as unfinished on any page.

Worth keeping: the first attempt drew the banner's boxes *on the news card*, because one function
draws both and `art` there is the news card's rect. The banner keeps `BANNER_ART` points at its own
foot for exactly this purpose; the rect is built from that now.

### The rail names itself — 0.1.35

Hovering a destination now shows its name in a dark pill beside the icon, as the reference does.
Drawn at the end of the frame, not with the rail: the rail is painted before the content column, so
the first attempt left the pill underneath the library cards — the same reason the toast is drawn
last.

### The tag overflow chip — 0.1.36

A card now stops its tag row where the stats begin and says how many it could not show, which is the
reference's `+1` chip. Before this the pills drew on under the download count.

It is verified by a test rather than a capture: this VM's window is wide enough for four tags, so
the chip's path cannot be reached with the real feed here. `tags_that_fit` is the decision, and its
test covers the boundary, the exact fit, the too-narrow row and the empty row.

The numbered page strip remains owed, and the reason is now measurable: the reference's content
column ends near x=1250 while this build's ends at 1159, so the ~150 points a `1 2 … 953` strip
needs do not exist on this row. That is a column-width difference, not a pager one.

### The numbered page strip — 0.1.37

The strip the reference draws — `1 2 … 953` with the current page in a brand pill — is in place, and
the reason the earlier count version existed is now fixed at its cause: the search field took 62% of
the toolbar row where the reference's takes about half, which left 79 points for a pager that needs
about 150. The field takes 50% now.

`pager_items(current, pages)` is the decision and has a test: ends always shown, the page in hand
with one neighbour either side, an ellipsis where a number is skipped, and no ellipsis when the gap
is a single page.

Verified on the VM: `sodium` drew `1 2 … 9` with page one in the pill; pressing 2 drew
`1 2 3 … 9` with the pill on two and different cards beneath.

Still owed on Discover: the `Hide already installed` switch and the `Category` sidebar.

### Discover's own sidebar — 0.1.38

`Category` now takes the slot `News` occupies on the other pages, as the reference's Discover does.
The ten modpack categories come from `GET /v2/tag/category` on a worker, filtered to the tab's
project type -- `adventure`, `challenging`, `combat`, `kitchen-sink`, `lightweight`, `magic`,
`multiplayer`, `optimization`, `quests`, `technology`, which is the reference's list in its order.

**Verified on the VM:** the list draws under its heading and a pressed row turns brand-green, while
pressing the same row again clears it.

**Verified after that entry was written:** the press changes the cards. Pressing `Adventure` on an
empty search drew Adventure-tagged projects -- Cobblemon Official Modpack, COBBLEVERSE -- each with
an `Adventure` chip of its own, which is the filter naming itself.

The probe kept missing because the rows are laid out from the panel's own plan and were being
guessed at from a cropped capture; printing the row rects once (`adventure` at y 276-302) put the
click in the right place first time. The lesson is the cheap one: ask the code for the coordinate
rather than measuring a screenshot.

Two faults worth keeping: the request block was nested inside `if let Some(query)` so a press with
an empty box never reached the search, and `request` refused an empty query outright, which is right
for the default feed and wrong once a facet is chosen.

Still owed on Discover: `Hide already installed` and its switch, and the `See more` link under the
list.

### The rail names its hosting item — 0.1.40

The tooltip over that item said `Servers`, the heading of the page it opens. The reference's own
tooltip names its hosting product there instead, so this one says `Sift Hosting` — the word the
page's heading and the promo banner already use.

Found by accident while trying to capture Skins: the click landed on this item and the tooltip was
the evidence. **The Skins comparison is still not taken** — two attempts opened Servers and
Screenshots instead, and the reference's rail y-positions are not where my readings of its captures
put them. The next attempt should ask the reference for its rail rects the way this build's panel
now prints its own, rather than measuring a screenshot.

### Skins verified — 0.1.41

Captured beside the reference at last. The two agree on the heading (`Skin selector`), the left
preview with `Edit skin` under it, a `Saved skins` section holding an `Add skin / Drag and drop`
card, and a `Default skins` section.

What the comparison found, largest first:

| | Reference | This build |
| --- | --- | --- |
| Skins are drawn as | rendered characters, 3D in the preview with `Drag to rotate` | **shirt glyphs** — no skin texture is decoded |
| Header controls | the title alone | a search field, `Wide`/`Slim` chips and an `Add skin` button |
| `Default skins` | four previews with a check on the selected | the line `Sign in to your Minecraft account to use its default skins.` |
| Foot of the page | a demo-account bar with `Sign in to Microsoft` | nothing |
| Rail tooltip | `Skin selector` | was `Skins`; fixed here |

The first row is the real work: skins are PNG textures on a fixed atlas layout, and drawing them on
a body needs that layout plus a mesh. That is a slice of its own rather than a tweak, and it is the
same gap that makes the instance pages' skin-less look different from the reference's.

Reaching this capture took three attempts. Hovering the reference's rail and reading its own
tooltips gave the pitch -- `Screenshots` at y=250, not the ~314 my measurements suggested -- which is
the same lesson as the Discover rows: ask the app, do not measure the picture.

### Skins are characters now — 0.1.42

The page's own largest gap is closed: a skin draws as a character, composed from its 64x64 atlas by
`compose_front` -- six front faces into a 16x32 view -- and shown in the preview column and on its
card.

The skins are authored here, because the reference's are Mojang's defaults. Two are written as
string art with a palette beside them; a skin without art keeps the glyph, which is why only the
first card shows a character in the capture.

That fallback came out of a panic: `SKINS` holds more entries than `ART`, so the first build indexed
past the end on the second card and took the window down. The guard is one line and the reason is
worth keeping -- a list and its parallel data drift.

Still different from the reference on this page: the reference's preview is a 3D model with
`Drag to rotate` and the whole body is modelled, not a still front view; its header has no search,
chips or Add button; its `Default skins` shows four previews; and its foot carries the demo-account
bar. All of those are listed, not approximated.

### Two more authored skins — 0.1.43

Four cards now draw characters rather than three glyphs: the blue and purple palettes are authored
beside the first two, and `Lantern Keeper` and `Snowbound` show their own colours.

**Owed, with the reason measured:** the `Default skins` row still reads
`Sign in to your Minecraft account to use its default skins.` where the reference draws four
previews. The row is written and does not fit, because that section lives in the page's fixed strip
below the grid instead of inside the scroll area -- so whatever the window height, it has no room.
Reserving the height was tried and backed out: it left empty scroll below the grid without moving
the drawing. The fix is to put the section inside the grid's own scrolling column, which is how the
reference lays its page out.

### The default skins scroll with the cards — 0.1.44

The section moved into the grid's own scrolling column, so it has room: scrolling past the cards
reaches `Default skins` and its four characters, with the brand check on the one in use. The
reference lays its page out the same way — one column, saved skins then default skins.

The earlier attempt reserved the height without moving the drawing, which would have left empty
scroll below the grid; that was backed out before it shipped and is recorded in the 0.1.43 entry. A
reservation and its drawing belong in one change.

### Discover's sidebar is complete — 0.1.45

`Hide already installed` sits above the `Category` heading with a switch, the list shows four rows,
and `See more` appends the rest -- the reference's arrangement.

The switch needed data nothing had: **no instance recorded which project it came from**, in either
`instance.cfg` or `mmc-pack.json`. So the installer now writes `sift.project.id` beside Prism's own
keys (Prism ignores settings it does not know), the library reads it back, and the filter hides a
result only when that id matches. Instances installed before this have no id and are never hidden,
which is the honest behaviour rather than a guess from an instance's name.

Verified on the VM: the switch, the heading, four rows and the `See more` link all draw, with the
chosen category in the brand colour.

### Settings buttons press, and `Clear cache` asks — 0.1.46

`Control::Button` was drawn and never hit-tested, so every button on that page was decoration. The
standalone arm registers a press on the control's own rectangle now.

`Clear cache` is wired to an action because it is the one whose behaviour the source defines: the
first press asks (`Press again to clear the cache.`), the second deletes the launcher's HTTP cache --
refetched on demand, so only time is lost -- and reports the size that went. `button_pressed` holds
the rule and its test pins the three cases, because a button that cleared a cache on one press is the
wrong thing to learn by trying.

**Verified afterwards, in three presses:** the first press draws `Press again to clear the cache.`;
the second, with no cache, draws `No cache to clear.`; and with a 2 MB cache planted by hand it draws
`Cleared 1.9 MB of cached downloads.` and the directory is gone.

The tab was reachable all along -- my clicks were landing below its row. Printing `rail_rows`' own
output put Troubleshooting at y 743-773, where I had been aiming at 792.

**Three stale notes corrected**, which is worth more than it sounds: the quick-instance count and the
memory slider both claimed to be missing their sliders, and the feature flags claimed `Reset to
default` was absent. All three have worked for several releases, so the owed list was overstating the
gap.

### `Copy ad details` — 0.1.48

The button copies and says `Copied` with a check for three seconds, as the reference's does. What it
copies is this build's own line -- product, version, platform, data root -- because a launcher with
no ad surface cannot copy an ad's details, and a support report wants exactly that.

**A platform finding worth keeping:** `request_repaint_after(3s)` does not wake this app. The first
build set the deadline and the word stayed on screen for twelve seconds. The frames that bring it
back are requested directly while the word is lit; that costs three seconds of frames after a
deliberate press and nothing otherwise. Any future "wait then redraw" in this tree should assume the
same and ask for its own frames.

### The language list — 0.1.49

`Block::RadioList` draws the reference's locale list: one row per locale with a radio, the chosen
name in the brand colour and a dot inside its ring. A press stores the choice under the heading's own
key, so the select above the list and the list itself are one setting -- pressing `Deutsch` leaves the
select reading `Deutsch` and the ring on it, verified on the VM.

Owed on that tab still: the flag and coverage percentage each row carries in the reference, and the
search field filtering the list (it is drawn, not searched).

**An operational note:** the VM's disk went to 97% twice this session, both times because `target/`
had grown to 15 GB from debug builds. `rm -rf target/debug/incremental` took it back to 70% in one
command; the release tree alone is small.

### The feature flags filter — 0.1.50

The search field now filters: a case-insensitive substring, with the reference's
`No flags found matching` when nothing survives. Verified on the VM both ways -- `column` leaves one
row, `zzz` leaves the line.

Two placement faults fell out of it, both worth keeping:

- **A text field drew its hint twice.** `place_field` puts a real `TextEdit` with `hint_text` on the
  control's plate, and the plate painted the same words underneath. The plate is drawn with an empty
  label now.
- **A skipped row swallowed the block above it.** `gap` carries the previous block's height until the
  next block claims it; the skip arm zeroed it, so the field's own height vanished and the first
  surviving row was drawn on top of the field.

**And a workflow trap:** `push_one.py` runs `cargo test`, which builds the test harness rather than
the runnable binary. A capture from `target/debug/SiftUi` can therefore be one build behind the
source -- which is exactly what made the empty-state line look broken when it was not.

### The store's usage bar — 0.1.51

Measured from the launcher's own folders and drawn as the reference's bar: one rounded bar split by
folder, a total, and a legend of tinted dots with each part's size. Verified on the VM at
**1234.7 MB in use** -- Instances 419.6, Libraries 208.4, Java 103.9, Assets 502.7.

The split is this build's own and the legend says so. The reference divides a *content cache* into
`Unique` / `Shared` / `Orphaned`; this launcher unpacks packs into instances and keeps no content
cache, so those three figures could only be invented. Same bar, honest legend.

Measured once per open: walking the instances folder is tens of thousands of files.

Still owed on Resources: the cache limit as a number field with its unit, the two tooltips, and the
app directory's own field and picker (the button is drawn).

### The profile's fields — 0.1.52

The username and bio are fields now. The bio is this build's first multiline control,
`Control::TextArea`: a multiline `TextEdit`, the source's `160`-character limit, and its counter
under the field. Verified on the VM -- typing gives `15 / 160`, the field takes the brand focus ring,
and the row offers `Reset to default`.

The textarea takes the row's whole width and the row grows to carry the counter. Its drawing arm
paints nothing at all: a plate behind a `TextEdit` shows as a second rounded rectangle offset from
the field's own edge, which is what the first attempt looked like.

Still owed on Profile: the signed-out empty state that replaces the body, the lead paragraph's two
links (drawn as literal markup), and the upload/remove image buttons doing anything -- they press,
and nothing listens yet.

### Java `Detect` — 0.1.53

One button under the four location rows scans `<data>/java` for runtimes, reads each one's `release`
file for its version -- not a spawned JVM, which would stall the frame it was pressed in -- and fills
the row that names it. Verified on the VM: the Java 21 row took the real path and immediately offered
`Reset to default`; the report read `Found 1 Java runtime(s); filled 1 row(s).`

`Browse` is still owed: it needs a folder picker, and this build has no dialog surface yet.

### Open after this pass

| # | Region | What differs |
| --- | --- | --- |
| E-6 | Cards | still placeholders: the reference's card has an icon tile, a title with a follower glyph, a two-line summary, a tag row of badges and a footer of downloads/followers/updated, with an `Install` button at the right |
| E-8 | Search row | the field, the two selects and the count are drawn but inert; the reference's field takes focus, shows a caret and a clear button, and the selects open menus |
| E-9 | Rail | the five destination glyphs render smaller and dimmer than the reference's, which are 24 px at `--color-secondary` with a 40 px plate on hover |
| E-10 | Panel | the news card's height and the row rhythm need measuring rather than the current constants |

### Two fidelity gaps in the icon set, recorded before they cost time later

`tools/gen_icons_egui.py` emits 441 glyphs / 1,734 shapes, and cannot express two thirds of
the reference's *filled* elements faithfully:

- **16 glyphs have holes or concave outlines** (`cloud`, `x`, `cog`, `spinner`,
  `radio-button`, `minecraft`, `vanilla`, `velocity`, `tags/categories/{entities,mobs,potato}`,
  `tags/loaders/{datapack,babric}`, …). epaint's `PathShape` has no fill rule and fills with a
  triangle fan from the first point, so a hole is filled solid. Fix: triangulate in the
  generator (even-odd) and emit a `Shape::Mesh`.
- **413 of 441 glyphs use `stroke-linecap="round"`**, which `PathStroke` cannot honour; the
  zero-length dot idiom was already mitigated by emitting discs, but joins still miter where
  the reference rounds them. Fix: insert small discs at joins in the generator.

Both are generator work, not app work, and both are visible at 32 px and above — which is
the size the rail and cards use.
