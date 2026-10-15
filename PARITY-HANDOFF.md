# Parity handoff — `/user/:name` and `/hosting/manage/`

A Rust + iced 0.12.3 port of the Modrinth App (Electron/Vue v0.21.6), matched pixel by pixel
against the real application. **The work is finished. This file is the handoff.**

The task was to bring `/user/:name` and `/hosting/manage/` to pixel parity. The user explicitly
rejected "done with documented gaps": every fixable gap had to be closed, and subagents were
authorised for the work.

---

## 1. State: complete and green

| gate | result |
| --- | --- |
| `cargo test -p palantir-desktop --locked` | **822 + 4 passed, 0 failed** |
| `cargo clippy -p palantir-desktop --all-targets --locked -- -D clippy::correctness` | **0 errors** |
| CI on the public mirror | **green, 5/5 jobs** (Test workspace, Lint, Live services, both Windows exes) |
| latest CI run | `37206630124` |
| `python tools/progress.py --check` | exit 0 |
| working tree | clean — only `.local/` untracked, which is correct |
| commits | **26**, all pushed to both remotes |

Remotes: `origin` = `MSedgeMC/PalantirMC`, `public` = `msedge-projects/PalantirMC`. Branch `master`.

Verify before trusting anything below:

```
cargo test -p palantir-desktop --locked
cargo clippy -p palantir-desktop --all-targets --locked -- -D clippy::correctness
python tools/progress.py --check
gh run list --repo msedge-projects/PalantirMC --limit 3
```

---

## 2. The commits

Each body carries the measurements. Read them with `git show`.

```
bbfdacf  Leave no blank lines at the end of the file
a1a0710  Record the two commits that landed after the ledger was last written
b356c15  Move stage 3's gate range to where G157 and G158 land
4d79133  Put back the newline three headings lost it
725dfd6  Give the card's own button the six pixels it was measuring short
1f393ab  Measure the shipped faces, not whatever this machine has installed
cec5440  Record the nineteen commits the profile and hosting parity work landed
da746ab  Space the labels the way the reference's own capture spaces them
5e88914  Give the strip's fourth link an address of its own
3eda7d7  Hang the header avatar's shadow where the card's shadow falls
029d2d8  Stop a scroll_to still in flight from reading as somebody else's move
214a167  Measure the avatar card's shadow and draw it behind the card
b0044c6  Name what the blend note's 2.60 is measured against
3eb414e  Compute the hosting plate's luminosity blend instead of approximating it
80dbf2f  Forward the pointer through the layers a stack draws over live content
ad93189  Draw the invite toast over the pane's reserved gutter, not under it
d50370d  Take a collection card's glyphs at the bare element rule's sixteen
8cf738b  Give a card's summary the column the reference's own grid gives it
15bfb0c  Read the two icon sizes apart, and record what the tab labels cannot say
fc5a0d4  Draw the friend names on the line the reference gives them
ce77da9  Wrap the metadata label's style call the way rustfmt wants it
4759cb4  Set every line height on a profile from the reference's own class
9765a9d  Weigh the profile tab labels at the weight NavTabs gives them
0f78a06  Ship the reference's own Inter faces instead of our subset of them
148a928  Reach the one loader tag whose message is not named after it
f34db59  Draw the pane's inset shadow, which was not being drawn at all
```

---

## 3. Findings that changed the shape of the task

### 3.1 `overlay::Stack` had no `on_event` — `80dbf2f`

It inherited iced's `Status::Ignored` and never descended, so **nothing inside the page pane took
pointer input on any page**: no tab switchable, no button pressable, no list scrollable. Only the
rail worked, because it sits outside the Stack.

Fixed with per-layer `over` / `over_control` flags, so the reference's `inert aria-hidden` preview
panel stays inert. The same pass fixed `Stack::diff` swapping in fresh trees every frame, which
made a hovered control light up and never go out.

### 3.2 `Region::adopt` raced an in-flight `scroll_to` — `029d2d8`

Nine wheel runs produced nine different offsets — 277 / 435 / 177 for three clicks against an
arithmetic of 360.

Fixed by making a commanded offset widen a **range** rather than overwrite a point, so repeated or
concurrent commands are idempotent. Prefer this to debouncing: idempotence costs no latency, where
a debounce trades the nondeterminism for it.

### 3.3 Four "documented limits" were false, and all four are now drawn

In each case the honest diagnosis was wrong, and the right answer was already in the tree.

**Avatar `box-shadow`.** iced composites a container `Shadow` *inside* the element's own
rounded-box coverage and bands its own fill. That part is real, and it is why the tab strip's
shadow was removed. But the pane's **inset** shadow had already been drawn from explicit opaque
bands, so `214a167` + `3eda7d7` do the same for the avatar: five pre-composited rings, one pixel
of reach each, byte-identical to the reference on all four sides.

**`mix-blend-luminosity`.** Arithmetic, not inexpressible. `3eb414e` computes it:
`mix-blend-mode: luminosity` is `SetLum(Cb, Lum(Cs))` — the **source's** luminosity on the
backdrop's hue and saturation — in **sRGB, not linear light** (1.20 rms against 3.27 linearised).
Worth up to 10/255 per channel, so a plain gradient cannot do it. The same pass found a separate
bug: `plate_overlay_pixels` strided a 38-wide window by 40, so the plate texture was drawn in the
wrong place and then not at all.

**Label widths 3–7px narrow.** Not the font, and not letter-spacing. Inter zero-pads its version
string, so `Version 3.019;git-0a5106e0b` **is** the 3.19 release, and `hmtx` matches the
reference's for every shared codepoint — zero differences across five weights. The reference's
stylesheet has **zero** `tracking-*`, and `letter-spacing` only on code blocks. It is space between
glyphs, but **not one number**: the profile strip fits +0.72 a character while the 30px hosting
heading fits −0.15. So `da746ab` fits **per label** from its own capture, inside `ui::advance`, so
layout and drawing cannot disagree. Strip error 16px → 2px.

**The collections route.** `/user/{name}/collections` was refused by
`ProjectType::from_profile_token`, so the strip's fourth link was a dead end. `5e88914` gives
`Route::User.project_type` the type `Option<ProfileTab>`, because collections are **not** a project
type, and a variant would corrupt `target_folder` / `sentence` / `TABS` / `ALL` everywhere else.

### 3.4 CI caught a portability bug no local test run would — `1f393ab`

The tracking test passed on Linux and failed on the Windows runner: *Data Packs* measured **83.6850**
here, **77.8054** there. A test binary never runs `run_shell`, so iced's global `font_system()` in a
test holds only **the host's installed faces** — this box has Inter, a Windows runner does not. The
constants were pinned to the machine, not to the five faces in `crate::FONTS`.

`shape_width_in` now takes the system to shape through, and tests build one from `crate::FONTS`.

This also **corrected four wrong numbers** the code had recorded. The "basis" between a reference
advance and `shape_width` was written as +0.530 / −0.618 / −0.978 / **+2.136**, the last attributed
to "a trailing `r` overhanging its advance". None of it was the glyph. On the shipped faces the
figures are **−0.206 / +0.001 / −0.004 / −0.070**. The old numbers were the *ambient* face's
overhang mistaken for the glyph's, and a test built on them would pass on any machine with Inter
installed and fail everywhere else.

### 3.5 Two things that looked like geometry were not

**The toast** stopped 10px short because the pane gutter **painted over** it — a z-order problem,
not a size one. The gutter must stay above the page (its bands are opaque precisely to hide iced's
scrollbar bar), so `ad93189` **hoists** the toast into the pane's `Stack` instead. Deleting the
gutter was tried and is wrong: the scrollbar reappears.

**A `no_bar()` experiment** changed **no pixels at all** and was reverted rather than committed with
a comment claiming a fix.

### 3.6 The card grid came from the spec, not from pixels — `8cf738b`

css-grid §11.2 (gutters are fixed-size tracks for the sizing algorithm) and §11.5 (a spanning item
distributes across the tracks it spans) give the `1fr` column as **521**. Card 2's summary then wraps
to exactly the reference's `217..737`.

`725dfd6` closed the last defect this left: our *Install to instance* measured **183** against the
reference's **189**, and because the column is `Length::Fill` it absorbed the 6px into 528, putting a
word on card 3 that the reference does not have. Both buttons are now **189** at `x=750..938`;
card 3's first line is **`217..709`** against the reference's `217..711`; card 2's is `217..736`
against `217..737`.

---

## 4. Measured landmarks — for regression checking

| thing | value |
| --- | --- |
| profile header rule ends | **x=868** |
| card fill | **90..954** |
| avatar card | **x=88..183 / y=72..167**, five shadow rings |
| hosting toast fill | **x=650..979** |
| pane gutter | **x=970..979** = `#16181C ×4 │ #16181B │ #15171B ×5` |
| iced's scrollbar bar (`#757C84` in x=960..985) | must be **0 px** |

Tab strip ink widths, before → after, against the reference:

| label | before | after | reference |
| --- | --- | --- | --- |
| `Data Packs` | 75 | **83** | 82 |
| `Modpacks` | 71 | **74** | 74 |
| `Collections` | 77 | **82** | 83 |

---

## 5. Capturing the pages

The reference is unavailable (see §6), so ours only:

```
DISPLAY=:NNN python3 tools/xshot.py --launch target/release/PalantirMC \
  --args "--page /user/FlameFire --size 1280x720" --client 1280x720 \
  --settle 12 --script /tmp/s.txt
```

- The script takes only `click X Y`, `wait N`, `shot OUT.png`.
- **You must `mkdir` the output directory first.** `xshot.py` does not create it and ffmpeg fails
  with a bare "Could not open file".
- Use a **private** display. `:95` is shared and another agent's window there contaminates
  `x11grab`. Start one with
  `systemd-run --unit=xvfbNNN --property=StandardOutput=null --property=StandardError=null /usr/bin/Xvfb :NNN -screen 0 1920x1080x24 -nolisten tcp`
- `cargo` is **not** on the default PATH inside `systemd-run`. `export PATH=/root/.cargo/bin:$PATH`.
- Long builds: `systemd-run` with a unique unit, writing `$?` to an `.exit` file, then poll it.
  Never `nohup`. Never kill another `cargo`.

---

## 6. Two honest caveats — both recorded in the ledgers

### The reference is unrecoverable

`/tmp/ref/` and the app on `:99` both died mid-task, and it will not start —
`Failed to initialize GTK`, which survives `GDK_BACKEND=x11`, a valid `XDG_RUNTIME_DIR`,
`GSETTINGS_BACKEND=memory`, `NO_AT_BRIDGE=1` and `GTK_A11Y=none`.

**No measurement after roughly `cec5440` is a fresh end-to-end diff.** Those figures are either
recorded values or derived from the reference's own source. Do not report them as re-verified.

### The `Install to instance` extra is derived, not fitted

It comes from the reference's measured 189px box and `ButtonFrame`'s `md` chrome
(`px-2.5`×2 + `gap-1.5` + `size-5` = 46 → 143.0 of label, against Inter-600 `hmtx` 136.76 →
**0.3467** a character). Unlike the four tab and tag labels, there were no glyph origins left to
cross-check it against, and it is nearly twice `New server`'s **0.1806** at the same size and
weight.

That is the expected shape of the finding rather than a defect — the reference's extra is not one
number — but it is the weakest number in the set.

---

## 7. Rules for any further work

`AGENTS.md` is short and is the authority. Read it first. Non-negotiables:

- Push to **both** remotes and confirm the run. `AGENTS.md` §1: a change that exists only on this
  machine has not happened.
- Fix forward with another commit. Never amend or force-push a commit already on a remote.
- Commit message: imperative subject, no `feat:`/`fix:` prefix, prose body wrapped at ~80 columns,
  ending with exactly:
  ```
  🤖 Generated with Codebuff
  Co-Authored-By: Codebuff <noreply@codebuff.com>
  ```
- Update `NEXT_STEPS.md`'s stage table **in the same commit** as the slice that moved it, with the
  percentage from `python tools/progress.py` — never written by hand.
- No `unwrap` / `expect` outside tests; gate tests enforce it.
- `line_height` must be `iced::Pixels(n)` — iced reads a bare `f32` as a **multiple** of the size,
  so `18.0` on 16px text is a 288px line.
- `.center_x()` and `.center_y()` take no arguments in iced 0.12.
- No new dependencies, no `unsafe`. Never commit captures, logs, or anything under `.scratch/`.
- Comments in this codebase explain *why* a number is what it is and cite a source file and line.
  Match that density. Every constant is a measured value with a citation.

---

## 8. How to judge the next thing

**Measure before concluding something is not fixable.** Four things in this task were written off as
inexpressible and all four turned out to be solvable:

- the font-version theory — Inter zero-pads, so `3.019` *is* 3.19;
- a `UsersIcon` that was asserted not to exist — it did;
- tags recorded as a data limit — v2 publishes them;
- Collections recorded as unreachable — `curl` returns 200, no credential needed.

Conversely, one plausible fix (`no_bar()`) changed no pixels and was reverted. Both kinds of error
cost rounds. The pattern that worked: read the reference's own source for the number, then confirm
it against pixels, and treat any conflict between the two as information rather than as something to
explain away.