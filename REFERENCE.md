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

Our own side first, because it is the half that can be repeated on demand:

```bash
# capture a page of this launcher at the reference's own client size
python tools/appshot.py --page home --size 1280x720 --out .scratch/pal-home.png
# and judge it against the numbers below
python tools/page_gate.py .scratch/pal-home.png --page home
```

`--shot` makes the launcher ask iced for its own frame and write it, so nothing
reaches into the window from outside and the capture is the frame this shell drew
rather than the compositor's rendering of a window parked off the desktop. (The
earlier way -- `winshot.py --park --method print` -- returned a black image on one
run and the page on the next, and could leave the launcher frozen on "Loading your
instances…": a `SetWindowPos` on a window whose runtime has not finished booting
blocks the thread that would have loaded them. The flag also removed the need to
click a rail entry to reach a page, which is what a capture on an unattended desk
cannot do.)


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
| Active entry | plate `#1d563f`, a **circle** on the 48px entry (see below) |
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

### The plate is a circle, and the rail's foot is its own cluster

Two things in that table were read wrong, and both are now settled by the pixels
*and* by the reference's own source, which is the strongest pair of readings
available:

- **The active plate is a circle.** It was recorded as "radius ~11-12 (**not** a
  circle)", from an arc through the plate's top-left corner. Counting the width of
  each of the plate's rows instead gives 14, 26, 30, 40, 44, 46, 48 — narrowing
  symmetrically back to 14 at the bottom. That is a circle of radius 24 on a 48px
  entry. A 12px radius cannot produce it: its first row would be 24px wide, and
  its widest row would begin 12 rows in rather than 17. `NavButton.vue` agrees —
  `w-12 h-12 rounded-full` with a selected state that is a `::before` at
  `inset: 0` with `border-radius: 50%` — and with the source and the shapes in
  agreement there is nothing left to fit.
- **The rail's foot is a cluster, and the eighth entry is the account.** The
  bottom four tiles sit at the foot of the rail, not after the fifth entry: a
  16px icon at y 576..591 (the create button inside `QuickInstanceSwitcher`), then
  Settings at y 624..647, then a 20px brand-green mark at y 678..697. That last one
  is `LogInIcon class="text-brand"` — the account entry while signed out, which is
  what the walk's "12px element at y 682..694" was. Their pitch is 52, the same as
  the top five, and they end 8px above the window's foot.

One more line from the source: the rail's divider is `mx-2 h-px bg-surface-5`
inside the rail's own 8px padding, so it spans x 16..47 — 32px, not the entry's
48 — and it sits 16 rows below the fifth entry (measured at y 320, where the fifth
entry ends at 304).

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

## The interactions, from the components that state them

A still capture cannot show a hover, and the reference's own source states every
one of these, so this part of the port is read rather than photographed. It is
its own table because the values are *rules*, not pixels: a `brightness()` is not
a rung of the palette and deriving one by hand is how this shell ended up with a
light theme that hovered the wrong way.

| Where | What it says |
| --- | --- |
| `assets/styles/variables.scss`: `.dark-mode` | `--hover-brightness: 1.25` |
| `ditto`: `.light-properties` | `--hover-brightness: **0.9**` |
| `ui/src/components/base/buttons/ButtonFrame.vue`, base classes | `transition-[background-color,color,box-shadow,filter,opacity,transform] duration-150 ease-out`, `enabled:active:scale-[0.97]`, hover and focus-visible `brightness-[--hover-brightness]`, `disabled:opacity-50` |
| `ditto`, sizes | `xs` 28px `rounded-lg`, `sm` 32 `rounded-[10px]`, `md` 36 `rounded-xl`, `lg` 40 `rounded-[14px] px-4`, `xl` 48 `rounded-2xl px-3.5`; label `text-sm`/`text-base`, weight 600 except `xl` at 800 |
| `ditto`, types | `base` = `bg-surface-4 text-contrast`; `colored` = `bg-[--button-color]` + `--color-accent-contrast`; `outlined` and `quiet` = no fill |
| `ditto`, interactions | `surface` = hover fills `bg-surface-4`; `filled` = hover keeps the fill and the contrast label; `none` = `hover:!brightness-100` |
| `assets/styles/classes.scss`, `.button-base` (the older component) | hover `brightness(0.85)`, active `brightness(0.8)`, disabled `grayscale(50%)` + `opacity: 0.5` |
| `app-frontend/.../onboarding-checklist/index.vue` | rows `hover:brightness-110 active:brightness-90`, complete rows `opacity-50`; accordion button `hover:brightness-110` |
| `app-frontend/.../library/instance-group/instance-card.vue`, `library/WorldItem.vue`, `library/InstanceItem.vue` | a card hovers at `brightness-110` / `[--hover-brightness:1.25]` / `1.1` |

The shell applies the two `--hover-brightness` values and the `opacity-50`
(`theme::hover_brightness`, `theme::DISABLED_OPACITY`) as one filter over a
control's whole appearance — fill, label and ring — because that is what a CSS
filter on an element does. Three things in the table are **not** drawn, and the
reason is the toolkit rather than the reference: the 150ms transition (iced has
no transitions; a state change lands in one frame), the `scale-[0.97]` press
(a widget is laid out and then painted, and cannot paint itself 3% smaller), and
the per-component factors on the right-hand rows of the table — a card's hover is
a `MouseArea`-shaped change this shell has not built yet. The press uses
`classes.scss`'s `brightness(0.8)` instead, which is the same intent as the 0.97
and is the value the dark palette had already derived by hand.

## The token gate, or: why these numbers are not just written down

Every value in the tables above started as a comment in the source saying which
token it came from, and a comment cannot fail. `crates/palantir-desktop/src/reference_tokens.rs`
is the check: it parses the two sheets below by selector, follows `var()` chains
to the end, reads a `linear-gradient()` as its stops, and compares the result with
what this shell actually paints -- the palette in all three modes, the radii, the
interaction factors and the type scale. A disagreement prints our value, the
reference's, and the file and line the token is declared on:

```
palette.surface [Dark]: ours #26282d, reference #27292e
    vendor/modrinth-app/assets/styles/variables.scss:237  --surface-3: #27292e
```

It reads `--name: value;` blocks out of:

| File | What it holds |
| --- | --- |
| `assets/styles/variables.scss` | `.light-properties` (which `html` extends), `.dark-mode`, `.oled-mode`, `.retro-mode`: every surface, text colour, radius, gap and interaction factor |
| `assets/styles/defaults.scss` | `body`'s font-size ladder and weight tokens, and the `--font-standard` family the type is set in |
| `ui/src/components/base/buttons/ButtonFrame.vue` | the interaction a button has, which is not a token: `duration-150`, `active:scale-[0.97]`, `disabled:opacity-50` |

Two things make it a gate rather than a report. A value that differs fails unless
it is a **declared deviation** with its reason attached, which is where the four
measured disagreements above live -- so "we deviate here" is a decision that has
to be written down rather than a silence. And the table cannot omit a value: the
palette's field names are read out of `theme.rs` itself, so a new colour is
unclassified until both files change. Running it with `-- --nocapture` prints the
other half, which is the part that is useful on a *passing* run: **189 tokens in
the reference's sheets, 29 held, 160 not held** -- the list a page pulls from as
it ports. That first number was 148 before this pass, when the report counted
only the three maps it built (`.light-properties` and the two modes that override
it); the reference's light mode is also the `html` block -- gaps, radii, the ad
colours, the ring -- and `body`'s type ladder, which a page will need just as
much. It now counts every name either sheet declares, and the fact that the count
is the same 189 the vocabulary copy below finds is two readers agreeing about the
size of the thing being ported.

## The whole vocabulary, copied and checked by a second reader

Everything the two sheets declare is now held by the shell, not just the 26
values the palette paints. `tools/gen_tokens.py` reads them at the commit
`UPSTREAM.md` pins, builds the reference's own cascade from its `@extend` lines,
follows every `var()` chain and writes
`crates/palantir-desktop/src/theme_tokens.rs`: **189 tokens in each of four modes**
-- light, dark, OLED and retro -- 144 of them colours, with the file and line each
is declared on.

It is a copy, not a transcription, because the sheets are read a second time
before it is trusted: `crates/palantir-desktop/src/reference_vocabulary.rs`
re-derives the same cascade in Rust, with parsers that share no code with the
generator, and compares all 756 rows key by key. A disagreement prints the file
and line the reference states the token on, and the fix is `python
tools/gen_tokens.py`. Two more tests are a third opinion -- the declared set
collected by a line scan, so neither parser's blind spot can hide a token -- and
the receipt that the values the palette gate checks are in the copy. All three
skip when the vendored tree is not checked out, which is `UPSTREAM.md`'s promise
that removing it changes no test.

The copy has a second half. `SCOPED` holds **139 declarations in 21 files**: the
tokens a component, a page or `classes.scss` sets for one selector, which have no
mode to resolve in and no cascade to merge into. They are the values that do not
read like a design system -- `--top-bar-height: 3rem` and `--left-bar-width: 4rem`
on `.app-contents`, `--ease-out-expo` on `:root`, the `--os-*` scrollbar knobs a
combobox configures, `--user-avatar-badge-size` computed from the avatar's own
size, the medal-promotion colours in `global.scss`, and the per-card hover
factors (`.instance-item`, `[--hover-brightness: 1.1]`) -- and they are exactly
the numbers a port needs and cannot get from the global cascade. Each row keeps
the selector that sets it, so the answer to "what is this number for" is a rule
name rather than a guess.

Those rows are held to the same standard: the tree is walked a second time in
Rust, the files are parsed by a reader that shares no code with the generator, and
the two readings are compared keyed by file and line. The control -- one byte of
one row in `tailwind-utilities.css` -- fails it with the file, the line, the
selector and both values.

The copy is compiled for tests only. It is what states, in the shell's own
source, which 163 tokens are still waiting for a page.

## The title bar, measured again

The first pass read the bar as three separate 48px headers, one per column. It is
not: it is **one chrome bar across the whole width** -- x 0..1279 at y 0..47 all
measures `#27292e` -- with a **1px `#42444a` rule** at y 48 across it, and only
then do the three columns begin (the page surface from y 49). The page column's
top-left corner is cut with a **20px radius**, which is the reference's own
`--radius-xl` (`1.25rem`) on `.app-contents` -- `border-top-left-radius:
var(--radius-xl)` -- and what a fit through the corner's own rows gives: at x 72
the first page-coloured row is 53, at x 80 it is 50, and at x 82 it is 49. Solving
the arc for r at x 72 (8 rows in) gives r = 20 and not the 16 an earlier reading
said; at 16 the row at x 72 would be 55.

Its contents, left to right, all measured on the Screenshots page at a pinned
1280x720 client:

| Thing | Measured |
| --- | --- |
| Logo mark | 28px, x 12..40, `#00da75` |
| Wordmark "Modrinth" | x 50..162, cap 19px (its own art, not type) |
| **Back** | a 30px outlined circle, x 175..204, ring `#37393e`, a left-pointing filled triangle inside, dim |
| **Forward** | the same at x 224..253, a right-pointing triangle |
| Page icon | 18px, x 257..274, `#afbac4` |
| Page title | 16px semibold, white, from x 283 (`Screenshots` ends at 377) |

The two circles are the find of this pass. The earlier walk recorded them as
"an unidentified dim pair of hollow glyphs" because OCR sees nothing in a ring;
reading them at 1px shows a back and a forward arrow, both dim because neither
has anything to do -- the reference's rail navigation does not push history, and
only in-instance navigation does. They are **not ported**: this shell's pages are
flat, so a history stack here would be a new feature wearing another launcher's
chrome, and the honest version of that is a change of its own (`NEXT_STEPS.md`
§24).

The bar's own title is also where the reference keeps the *page's* name: the
Screenshots page draws no heading of its own, and neither does Home (its bar
reads `Home`). Discover's bar reads `Discover modpacks` -- the page and its
current tab -- so the bar is a running statement of where you are, which is what
ours now draws too (`Page::title()` and `Page::icon()`).

### The head, from `App.vue` and then from the pixels

`App.vue`'s `.app-grid-statusbar` is the whole thing, and it is short enough to
state completely. `bg-bg-raised`, `h-[--top-bar-height]` (3rem, 48px),
`padding-left: 0.25rem` around a `p-2` section, and left to right:

| Element | Its own classes | Measured in the reference's window |
| --- | --- | --- |
| `TextLogo` | `h-7 w-auto shrink-0` | ink x 12..35, y 10..36 — a 28px box at x 8 |
| back | `IconButton type=outlined`, `!h-7 !min-w-7 !w-7 !border !border-surface-4 !p-0`, `gap-2` | a 28px ring at x 172..200, ink y 9..38 |
| forward | the same | the same at x 208..236 |
| `Breadcrumbs` | `pl-4`, entries `gap-1.5`, `text-base font-medium leading-6` | a 20px visual at x 253..273, label from x 279 |
| right section | `flex shrink-0 ml-auto items-center` | the sidebar toggle (`mr-3`), `AppActionBar` (`mr-3`), `WindowControls` |

Two things follow from that table and neither was in this shell before this
change:

- **The head has no search field and no refresh button.** The reference's search
  is the *library's* own toolbar — `LibraryToolbar`'s first row is an `Input` with
  `wrapper-class="min-w-[16rem] flex-1"`, its placeholder is `app.library.search.
  placeholder` (`Search`), and the same page's heading is `app.library.library`
  (`Library`) over `flex flex-col gap-3 pb-16`. Its refresh lives in
  `AppActionBar`, in the head's right section, next to the running-instance chip.
  So the instance search moved to Home's toolbar, which is where the reference puts
  it, and the bar draws the two rings instead.
- **The two rings are dim, and they are dim in the reference's own build too.**
  Its rail navigation does not push history, so neither has anywhere to go. The
  source's `:class="{ 'opacity-20': !canNavigateBack }"` would put a disabled
  chevron at roughly `#292b2f` over the chrome — all but invisible — while the
  installed build paints `#96a2b0`, its `--color-text-tertiary`. The visible one is
  copied. The ring itself measures `#404248` flat, where the source's
  `--color-button-border` (`rgba(193, 190, 209, 0.12)`) over the chrome resolves to
  `#3a3b42`; the pixels win, as everywhere else the two disagree, and that is the
  third such disagreement after the accent and the brand highlight.

### The band that was above the head

This shell wrapped its whole window in a 6px band of resize grips on every edge,
and the top one was painted in the page's colour. It was 6px of the window that
was not the head: the head began at y 6 and put its rule at y 54, where the
reference's head begins at y 0 and its rule is at y 48. Both windows drew every
part to the same numbers and could never line up, which is the kind of difference
a gate measuring either window alone would never see.

It is gone from the top and kept on the other three edges, which is safe because
the band was only ever a fallback: the `native` hit test already answers
`y < RESIZE_BAND` for the top edge (see `native::edge_at`) and the head is a drag
patch across its whole width. The measurements above are now the reference's own
to the pixel — the rule at y 48, the pane's corner rows at 62/53/50/49 and the
plate's rows at 14/26/44/48/44/26/14 — which is how the removal was checked.

## The Screenshots page, measured

Session 05 walked the reference to this page and captured it; sessions 06-08
measured what moves. The client was pinned to **1280x720**, so every number below
is a client pixel at device scale 1.0.

| Thing | Measured |
| --- | --- |
| Page column | x 65..979 (`#16181c`), from y 49 down |
| Illustration | x 409..624, y 296..408 -- **216x113** |
| Its fill / outline | `#1d1f23` (6151 px) / `#34363c` (1230 px) |
| Heading ink | rows 462..484 (23 rows: cap 18 + descender), pure `#ffffff` |
| Subtext ink | rows 497..512 (16 rows), `#95a2af` |
| Gap, art -> heading | 54px of ink (408 -> 462) |
| Gap, heading -> subtext | 35px of ink (462 -> 497) |
| Content centre x | 516.5 -- the column's middle is 522.5, i.e. an **11px** scrollbar gutter the centring ignores |
| Content centre y | 404.5 -- the column's middle is 384.5, i.e. **20px below it**, which is what a content box 40px taller than its viewport produces |

The sizes behind those ink rows were confirmed rather than assumed, by the width
of the strings in the Inter face this shell already ships: "No screenshots yet"
measures 223px of ink against 227px at 24px bold, and "Screenshots you take
in-game will appear here." measures 358 against 361 at 16px regular.

The page draws **nothing else**: no heading, no rule, no button, no drop target.
That is the whole of the cluster, and `tools/page_gate.py` asserts it as one.

### How close the port landed

Measured off the build CI produced (`d23ed32`) on the same 1280x720 client, with
`tools/page_gate.py` as the judge — 9 met, 0 unmet:

| Thing | Reference | This launcher |
| --- | --- | --- |
| Illustration box | 216x113 | 213x108 |
| Heading ink | 23 rows, `#ffffff` | 23 rows, `#ffffff`, 226px wide |
| Subtext ink | 16 rows, `#95a2af` | 16 rows, `#96a2b0`, 362px wide |
| Gap, art -> heading | 54 | 53 |
| Gap, heading -> subtext | 35 | 37 |
| Content centre | x 512.5, y 400.0 | x 514.0, y 390.0 |

The illustration is five rows short of the box because the port's own back frame
starts 6px inside it and its front frame ends a pixel early — within the gate's
tolerance, and left as measured rather than tuned. The two gaps land within a pixel
and two, which answers the question the port could not answer from pixels alone:
the gaps are stated box-to-box (47 and 7) because a text widget's box begins above
its cap, and iced's boxes do carry the descender without a line gap, so the ink
lands where the reference's does.

### What is not copied

- **The artwork.** The measured box, palette and gaps are copied; the drawing
  inside is ours (three cascaded picture frames rather than the reference's fan).
  Modrinth's illustration is its own art and this repository ships no third-party
  art without a notice.
- **The Refresh chip.** This page used to draw an in-page "Screenshots" heading,
  a rule and a Refresh button, none of which the reference has. Removing them is
  what let the page be one cluster; the rescan they drove still runs on entering
  the page, which is when the reference's own page reloads too.
- **The populated grid.** The reference's own data root has no instances, so its
  Screenshots page is only ever the empty state here. What it draws once there
  are screenshots is unmeasured, and ours keeps the grid it already had.

### What moves, and what does not

Measured with a new `burst` verb in `tools/refwalk.py` (raw captures a fixed
interval apart, where `shot` would OCR each frame and cost two seconds):

- **The page does not fade.** Twenty-six frames starting at the click show the
  Screenshots page fully drawn in the *first* frame -- same heading ink, same
  ink box, same brightness as the settled capture. The page switches; nothing
  animates. (An earlier session mistook 1-level antialiasing differences between
  captures for a fade. It is not: the reference antialiases text in colour
  (ClearType), so glyph edges read differently from ours, which is also why the
  page gate compares modal ink colours and ink boxes rather than glyph bitmaps.)
- **The page has no hover.** The title, the empty state and the panel's header
  were each hovered and each capture differs from the unhovered one only by that
  same subpixel noise.
- **The rail plate is instant** at 40ms sampling, and the rail tooltip appears
  inside the first 1.4s of the burst -- no transition to measure.
- **The right panel is the animated part**: its promo block (x 997..1279,
  y 479..710) changes between captures two seconds apart with nothing hovered,
  and its card area rotates too. Neither is ours to copy -- both are Modrinth's
  own promotions -- but it is why a whole-window diff of the reference is useless
  as evidence: every panel-inclusive comparison carries motion that has nothing
  to do with what is being measured.

## The Home page, measured

The densest page, and the one whose element vocabulary every other page reuses.
Measured off `.scratch/ref-01-home.png` (a 1280x720 client, device scale 1.0) and
cross-checked against the component that draws it: `WelcomeScreen.vue` in
[`modrinth/code`](https://github.com/modrinth/code) states the layout in utility
classes, and where the two agreed the capture is what is recorded here, because ink
is what a capture can be asked about.

| Thing | Measured |
| --- | --- |
| Illustration | y 219..318 on x 463..562 -- **100x100**, the reference's `size-25` slot |
| Its fill / outline | `#1d1f23` / `#34363c`, the pair the Screenshots art uses too |
| Title ink | rows 348..367, 24px semibold `#ffffff` |
| Description ink | rows 389..404, 16px `#b0bac5`; its core ink reads `#afbac4` |
| Brand button | y 431..470 (**40 tall**), x 410..615, fill `#00da75`, label `#000000` |
| Hint row | y 487..506 (**20 tall**); key cap 18x18 of `#34363c` inside a `#42444a` ring |
| Prompt ink | rows 621..634, 14px `#96a2b0` |
| Import button | y 653..692 (**40 tall**), x 403..622, `#34363c` inside `#42444a` |
| Gaps, ink to ink | 30 (art -> title), 22, 27, 17, 19 |
| Hero centre | 18px above the column's middle, with the bottom block pinned 24px off its bottom |
| Everything centred on | x 512.5 -- the column's middle minus the same 11px scrollbar gutter |

The page draws **seven** things and nothing else: no card, no in-page heading, no
rule. That is what makes the old build's version fail the gate's first assertion --
ours drew a welcome card holding the hero, which paints as one band from y 124 to
y 599.

**Two of the reference's own numbers disagree with its stylesheet, and the
stylesheet is right about the layout.** The hero is centred in the space *above* the
bottom block rather than in the page (`WelcomeScreen.vue`: the hero is `flex-1`, the
bottom block is its sibling), which is exactly the 18px above the middle the capture
shows; and the two colours the 0.204 build paints -- `#00da75`, and the `#1d563f`
rail plate -- are not the token ladder in `main`, which says `#1bd96a`. The capture
is what is copied.

**A thin line on a dotted backdrop is not one colour.** The reference paints a
dot texture over this page (415x478 dots, each pixel up to 20 levels off the page),
so a 16px regular line's most-common colour at any threshold is a half-lit edge:
`#34383d` for a line whose token is `#b0bac5`. The gate asks what the *brightest*
ink is within 12 levels of the strongest pixel instead, which answers `#afbac4` --
and that is also why the illustration's own fill cannot be measured at a threshold
that excludes the dots: at 21 levels it is *fainter* than they are, so the gate
finds its box by asking which rows are mostly ink across its width rather than which
rows are bright.

### How close the port landed

Measured off this shell's own capture at the same size (`tools/appshot.py --page
home`), with `tools/page_gate.py --page home` as the judge — **12 met, 0 unmet**:

| Thing | Reference | This launcher |
| --- | --- | --- |
| Illustration box | 100x100 | 100x100 |
| Title ink | 20 rows, `#ffffff` | 20 rows, `#ffffff` |
| Description ink | 16 rows, `#b0bac5` | 16 rows, `#b0bac5` |
| Brand button | 40 rows, `#00da75` + `#000000` | 41 rows, `#00da75` + `#000000` |
| Hint row | 20 rows, 18px cap | 20 rows, 17px cap |
| Import button | 40 rows, `#34363c` in `#42444a` | 41 rows, `#34363c` in `#3d3f45` |
| Gaps | 30 / 22 / 27 / 17 / 19 | 30 / 24 / 29 / 17 / 21 |
| Band centres | x 512.5 | 512.0 to 513.0 -- every offset under a pixel |
| Hero above the middle | 18 | 18.0 |

The two gaps that differ by 2px are the two lines the reference sets with a line
box taller than its cap; ours carry the same descender without the half-leading, so
their *boxes* are 2px shorter and the ink below them starts 2px higher. That is the
model the Screenshots port already stated -- gaps are box-to-box here, expressed as
the ink distance the reference's own box produces -- and it is now confirmed on a
second, denser page rather than assumed.

### What is not copied

- **The illustration.** The reference draws its own square brand mark in this slot.
  The measured box, its two colours and its corners are copied; the drawing inside
  is ours (the launcher's hourglass on the measured plate, `glyphs::welcome_art`).
- **The right panel's promos, the "Getting started" list and the account card** --
  all captured, all Modrinth's own content or another launcher's features.
- **Their copy.** "Welcome to Modrinth", "Modrinth" in the title bar and the
  reference's own strings stay theirs; ours say this launcher's name in the same
  positions, at the same sizes, in the same colours.

## The Discover page, measured and ported

Measured off `.scratch/ref-02-page-1.png`, a 1280x720 capture from the first rail
walk, and recorded here because the port is per page and this is the next one. What
is *not* here is the card's interior rhythm: the icon, title, author, description,
tags, stats and Install button each need their own pass, and guessing them would be
the thing this file exists to prevent.

| Thing | Measured |
| --- | --- |
| Page column | x 60..941, page `#16181c` |
| Tab strip | a `#27292e` plate, x 84..690, **y 68..114** (47 tall), its pills inset 6px |
| Its pills | 35 tall, 20px of side padding; "Modpacks" is x 89..192 (104 wide) |
| The selected pill | fill `#1d5540` (`--color-brand-highlight`) |
| Its labels | 14px white; Modpacks x 109, Mods x 198, Resource Packs x 255, Data Packs x 424, Servers x 621 -- ink rows 88..95 |
| Search field | fill `#34363c`, x 84..941, **y 122..170** (48 tall), placeholder 16px |
| Sort / View row | two controls, x 84..339 and x 348..491, **y 178..213** (36 tall) |
| Result count | right-aligned at x 812 and 869 on the same rows -- "2" and "917" |
| Gaps | strip to field **7px**, field to sort row **8px**, both box to box |
| Cards | x 84..941 (858 wide), **140 tall**, a **14px** gap between them, fill `#27292e`, radius ~14 |
| First card | y 227..366, then 381..520, 535..674, 689..828 |
| Card icon | 100x100, 16px from the card's top and 17px from its left |
| Install button | ~94x36 at x 831..924, y 242..278, label `#00da75` with a `+` glyph |
| Card title | 20px at x 218; author 14px at x 436 on the same line |
| Card description | 16px at x 218, wrapping (y 278 and 296) |
| Card stats | 16px at x 771..928, y 305 |
| Card tag row | 16px at x 246, y 334; "2 days ago" right-aligned at x 847 |

Four of those numbers are corrected from the first pass, and the corrections are
worth keeping because they were the earlier pass's mistakes, not rounding:

* the strip is **47** rows tall, not 33. Its pills are 35 rows inset 6px in the
  plate; the 33 was measured between the two rows where the plate's rounded ends
  are at their widest, which is 14 rows short of the plate.
* the field is **48**, not 42, and starts at 122 rather than 128: the earlier
  numbers were the *placeholder's* ink rows read as the field's box.
* the gaps are **7** and **8**, not 17 and 16 -- the earlier pass measured ink to
  ink across two text boxes, which is a line-box question this page does not
  have, because a strip and a field are filled shapes.
* the sort row is **36** tall, not 28.

The port was written to the first (wrong) set and then corrected to the second by
`tools/page_gate.py`, which measures both clients the same way and states the
gaps -- that is the whole reason the page gates exist.

**Two things this page does differently from the two ported before it** (the
Screenshots page and Home): its right panel is a *filter* column -- "Search content...", Environment (Client / Server),
Game version, Open source -- and it is **331px wide against the 299px** every other
capture shows, so the panel's width is the page's business rather than the shell's.
And the page itself has no in-page heading at all: the tab strip is the first thing
in the column, and the page's name is in the title bar (`Discover modpacks`, with
the active tab in it).

**Where this launcher stands, after the port.** A capture of our Discover page at
the same size (`tools/appshot.py --page browse --out .scratch/pal-discover.png`)
now measures: strip y 70..117 (48 rows, 545 wide), gap 7, field y 125..171 (47
rows, 879 of the column's 903), gap 8, sort row y 180..214 (35 rows). Its title
bar reads `Discover modpacks`, its first thing in the column is the plate rather
than a heading, and there is no Search button.

**What the port draws, and what it does not.**

* **The strip.** The plate is the *row of tabs itself* rather than a container
  around them: each pill is filled `--surface-3`, the selected one
  `--color-brand-highlight`, the seams between them are square and only the two
  outer ends keep the 12px radius. That is what the reference's own pixels show,
  and it is also the only shape this toolkit measures correctly -- a container
  wrapped around the row hands its child a bound derived from the column's
  cross-axis size (0 for a shrink-width column), and every label collapses to
  nothing. That was captured twice, at 22px and 40px wide pills, before the
  container came out.
* **The Sort control** is real: `browse::Sort` carries the reference's five orders
  -- Relevance, Downloads, Follows, Newest, Updated -- as Modrinth's own `index`
  values, and picking one re-runs the search rather than re-sorting what is on
  screen.
* **No View control.** The reference toggles its results between a list and a
  grid; this shell has one result layout, and a control that toggles nothing is
  worse than its absence.
* **The count** on the right is what this client kept (`SEARCH_LIMIT`), not the
  API's `total_hits`, which the search response carries and `parse_search` does
  not yet read.
* **A card's icon slot** is the measured 100x100 box holding the content type's
  glyph. The reference loads each project's own icon there, which is a network
  fetch per card this shell does not do yet.
* **The right panel is still this shell's.** The reference's Discover panel is a
  *filter* column 331px wide -- "Search content...", Environment (Client /
  Server), Game version, Open source -- against the 299px every other page's
  panel measures. Porting it means porting the filters themselves (facets on the
  search, a different panel width per page), which is its own change rather than
  a page's.
* **Servers** is still absent from the strip: the reference has a sixth tab for
  it, and `project_type:server` answers 0 hits through the public search API, so
  a tab for it could only ever be empty.

## Surfaces measured, with what they contain

- **Home** — hero "Welcome to Modrinth" / "Ready to start playing?" with a
  `#00da75` "+ Create an instance" button, "Press N to quick create an
  instance", and an "Escaping another launcher? / Import from launcher" row.
- **Discover** — six tabs, a search field, Sort and View dropdowns, and results
  as full-width 106px cards (icon, title, author, description, stats, loader
  chips, version, "Install").
- **Skin selector** — in-page heading, "Saved skins" and "Default skins" grids,
  "Add skin", and a drag-and-drop target.
- **Screenshots** — as measured above: one centred cluster, the page's name in
  the title bar instead of a heading, and no controls.
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

0. **The bar.** It now names the page it belongs to -- an 18px glyph in the
   secondary tint, 9px, then the title at 16px semibold white, all three
   measured -- and it is one drag patch like the brand, so the strip under the
   pointer is never dead. `tools/page_gate.py` is the page-content oracle:
   reference numbers in, one capture in, a verdict out.
1. **The accent.** `#00da75` replaces `#1bd96a`, with hover and pressed derived
   by the rule the palette already documents (`brightness(1.25)` / `0.8`).
2. **The rail.** The active plate is a 12px-radius `#1d563f` square, not a
   circle, and the idle icon is `#afbac4`. The container is 64px.
3. **The order.** Home, Discover, Screenshots come first, in the reference's
   order and with its tooltips; the instance-scoped pages stay behind the
   divider, where the reference keeps its own instance content.
4. **The tab strip.** Modpacks leads, and the strip carries the reference's
   order for the types we serve.
5. **The Screenshots page.** The in-page heading, the rule and the Refresh chip
   are gone; what is left is the measured cluster -- the 216x113 illustration,
   the 24px bold white heading, the 16px tertiary subtext -- centred 20px below
   the column's middle and 11px of gutter left of it, exactly where the
   reference's own content lands.

## The loading page, ported

The reference's splash is `components/ui/SplashScreen.vue` and
`components/ui/ProgressBar.vue`, and it is a *layer stack*, which is why it is read
from those files rather than from a capture: a capture of a stack reports the flat
result, and the tokens are what state what the layers are.

| Thing | The reference's own numbers |
| --- | --- |
| The column | centred on the window, `gap: 1rem` (16px), colour `--color-contrast` |
| The wordmark | `height: 2.25rem` (36px), `width: fit-content` |
| The bar | `max-width: 20rem` (320px), `height: 0.5rem` (8px), radius `--radius-lg` |
| The bar's track | `--color-button-bg` -- `#34363c` in dark |
| The bar's fill | `--color-brand`, and `width` transitions over 0.3s ease-out |
| Its ramp | `fakeLoadingIncrease()`: +2% every 5ms, stopping at 95 |
| The minimum display | `MIN_DISPLAY_MS = 500` |
| The fade | `opacity 0.3s ease-in-out`, then the app appears |
| Layer 1 (bottom) | `--color-bg` (`#16181c`), opaque, full window |
| Layer 2 | the same colour with `cube.png` centred at 180vw x 180vh, `opacity: 0.8`, blend `normal` |
| Layer 3 | `linear-gradient(180deg, rgba(66,131,92,0.275) 0%, rgba(17,35,43,0.5) 97.29%)` |
| Layer 4 (top) | `linear-gradient(0deg, rgba(22,24,28,0.64), rgba(22,24,28,0.64))` over layer 3 |

**What the port draws.** The same column at the same sizes, over those four layers
resolved into one gradient -- iced 0.12 has no z-order to stack them with, so
the two gradients are composited over `--color-bg` by `theme::splash_sample`
(source-over twice), which answers `#1a2322` at the top of the window and
`#151a1f` at 97.29% down it. The bar, the 500ms minimum, the 0.3s fade and the
ramp are the reference's algorithms, written as functions of the clock rather than
as timers.

**What is not copied.**

- **The cube artwork** (`assets/loading/cube.png`) is Modrinth's, and this repo
  ships no third-party art without a notice. The layer it sits in is drawn as the
  flat colour the reference lays it over; the port's background is therefore the
  reference's gradient without its texture.
- **The wordmark** in the 36px slot is this launcher's emblem and name, where the
  reference draws its own logotype.
- **The line under the bar** is this shell's status while the scan runs
  ("Loading instances..."), where the reference only fills that slot during a rare
  directory move.
- **Light mode** has its own three splash tints. This launcher's chrome still has
  no light palette, so the splash is written once, in the dark look.

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
- **The mark's ink.** The head's mark is a 28px box in both clients because
  that is the reference's `h-7`, but the two marks are different art and the ink
  inside the box is not the same size: the reference's spans x 12..35 and y 10..36,
  while ours spans 16 by 25 in the same box, because `logo512.png`'s own ink box is
  `107,23..404,487` of 512 — a tall, narrow mark. Redrawing ours to fill the box
  the way the reference's does is a change to the art, not to the head.
- **The rail's entries.** Its five are Home, Discover, Skin selector, Screenshots
  and Modrinth Hosting, and its foot is create, Settings, account. Ours are Home,
  Browse, Screenshots, the instances folder, and then create, Settings, Accounts,
  About — because this shell's Mods, Worlds, Logs and per-instance Settings live
  under an instance rather than in the rail, and the reference's Skins and Hosting
  have no counterpart here (see the note on the Skin selector above). The geometry
  is copied exactly; the list is not copied at all, and copying it would mean
  restructuring navigation rather than drawing a page.
- **The window controls.** The reference draws its own caption buttons inside the
  web view; this shell's are iced's, in the head's right section at the same place
  and size, with Snap Layouts routed through the non-client hit test rather than
  through a Tauri window-command.
