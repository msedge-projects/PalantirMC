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
