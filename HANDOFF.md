# Handoff — 2026-10-20, CurseForge, Bedrock and the Sift installer

This is the note for whoever picks up next, written at the stop of the session
that began the Sift work. It records **what is committed and green**, **what the
commit does not contain**, and **the seams the next three changes go through**.
Read `AGENTS.md` first; it is the working contract, and this file is a
continuation of it, not a replacement.

The short version: the CurseForge client, the Bedrock module and the installer
are in the tree and tested. **Neither the CurseForge catalogue nor Bedrock is
reachable from the window yet** — Discover still searches Modrinth alone, and
nothing draws a Bedrock section. The installer compiles; it has never been run
on Windows.

## 1. What is in the tree

| Artifact | Path | What it is |
| --- | --- | --- |
| CurseForge client | `crates/palantir-net/src/curseforge.rs` (~1170 lines) | Core API v1 queries, JSON, project/file models, the modpack manifest parser, and the two policy refusals below. Exported from `palantir-net/src/lib.rs`. |
| Bedrock | `crates/palantir-desktop/src/bedrock.rs` (400 lines) | `presence()`, `worlds(root)`, `packs(root, kind)` over the player's own `%LOCALAPPDATA%\Packages\Microsoft.MinecraftUWP_8wekyb3d8bbwe` data, plus `STORE_URL`. Read-only. Declared `mod bedrock;` in `main.rs`. |
| Installer | `installer/sift.nsi` (261 lines) | NSIS + MUI2: brand page that plays the animation, welcome, licence, components, directory, install, finish. Per-user install under `$LOCALAPPDATA\Programs\Sift Launcher`, HKCU uninstall entry, `App Paths` registration, optional desktop shortcut, runs the app from the finish page. |
| Installer mark | `installer/sift.ico` | BMP-entry ICO (NSIS refuses PNG-compressed entries). A placeholder of the current mark: **replace with Sift's own logo.** |
| Installer animation | `installer/anim/sift.avi` | 96×96, 24 frames, 25 fps, uncompressed BI_RGB, committed. Played by `SysAnimate32` via `ACM_OPEN`/`ACM_PLAY` on the brand page. |
| Animation generator | `tools/gen_installer_anim.py` | Writes the AVI container itself (no ffmpeg dependency on a build machine) and `--check`s it against the committed file. |

Two behavioural decisions inside the CurseForge client that the docs do not
state and a reader will otherwise try to "fix":

- **Nothing is cached.** Overwolf's API terms forbid storing what the API
  answers, so a search asked twice is sent twice. Do not route it through
  `engine::cache` to "match Modrinth"; that change would violate the terms.
- **A project with `allowModDistribution = false` is refused by name**, with its
  `curseforge.com` address, instead of being half-installed. The refusal is
  `CfMod::distribution_refusal()`; the client never guesses a download URL for it.

And one inside Bedrock: the Store package is licensed per account, so the module
**does not fetch or install the game**. It finds a data folder that is already
there, lists it, and for a machine without one hands out `STORE_URL`.

## 2. The gates, as measured at the stop

Run from the repository root. All of these were green on the committed tree:

```
cargo test -p palantir-net --lib                                  # 288 passed, 0 failed
cargo test -p palantir-desktop --bin PalantirMC                   # 865 passed, 0 failed
cargo clippy -p palantir-net -p palantir-desktop --all-targets -- -D clippy::correctness
rustfmt --edition 2021 --check crates/palantir-net/src/curseforge.rs \
                               crates/palantir-desktop/src/bedrock.rs
python3 tools/gen_installer_anim.py --check                       # exit 0, file matches
makensis -DVERSION=0.1.0 "$PWD/installer/sift.nsi"                # exit 0, setup exe in dist/
```

The `makensis` run above was against a **dummy `MZ` payload** standing in for
`target/release/PalantirMC.exe`, because this host cannot build the Windows
binary. It proves the script compiles and links; it does not prove the
installer runs.

The push that carried this file was answered on the runner by CI run
`39453976089`: **all five jobs green** — Lint, Test workspace, Live services,
and both Windows exes. That is the first time `bedrock.rs` and `curseforge.rs`
compiled into a Windows binary; it is still not evidence that the installer has
been run.

## 3. What is not done — the next three things, in this order

### 3a. CurseForge is in Discover now; installing from it is not

**Settled on 2026-10-24.** Discover's controls row carries a third combobox --
*Source*, with Modrinth and CurseForge -- and both catalogues read. What the
switch does, in the order a reader meets it:

- `pages/discover.rs` → `Source`, `cf_class`, `CatalogQuery`, and
  `State::curseforge_request`. The source menu offers only the tabs CurseForge has
  a class for (**Modpack, Mod, ResourcePack, Shader**); a tab press onto
  `Datapack`, `Plugin` or `Server` returns the page to Modrinth.
- `store.rs` → `Store::search_any(&CatalogQuery)` is the one door the shell calls;
  it dispatches to `Store::search` or `Store::curseforge_search`, and the key comes
  from `Store::curseforge_key()` (environment, then the prefs row).
- The CurseForge question carries the text, the class, the order and the page; a
  game version and a loader only when the reader chose **exactly one** of each,
  because its API takes a single value and the sidebar's sets are OR lists.
  Categories, licence, environment and *hide already installed* are Modrinth
  facets and are not sent. Relevance and Followers both map to CurseForge's
  *Popularity* order, its own default.
- A CurseForge card press opens `curseforge.com/projects/<id>` through
  `pages::Open::Url` -- the canonical address, and the one place this launcher
  navigates outside itself on purpose.
- The key has a **field** now: Settings → *APIs* (a ninth tab under a fourth
  heading, both this launcher's own -- the reference has no third-party
  credential). The input is masked, starts on the saved value, and Save trims it
  into `Prefs::curseforge_api_key`; typing writes nothing, and an empty field
  removes the row so `PALANTIRMC_CURSEFORGE_API_KEY` can win for one run. G166
  holds the four facts.

What is **not** built, and is the next slice in this area:

- **Installing from CurseForge.** No project page, no file rows, no
  `resolve_download`, no loader hand-off. That is why a card opens the site.
  `CfMod::distribution_refusal()` is the sentence a refusal will use when a file
  row exists to refuse.
- **A CurseForge project page.** `Route::Project` reads Modrinth by id; a
  CurseForge id in that route would be an empty page.
- **A live request.** No machine in this session had a key, so the whole path is
  proven against a scripted fetch. The first session with a key should run one
  search and compare it with what `curseforge.rs`'s parser expects.

### 3b. Bedrock is a page now, and it still installs nothing

**Settled on 2026-10-24.** `/bedrock` is a route, the rail carries a sixth place
beside Servers, and `pages/bedrock.rs` draws what `crate::bedrock` reads: the
presence (installed with the game folder's path, the Store button, Windows-only, or
the reason the folder could not be located), the worlds, and the two pack shelves,
each with the empty sentence that names the folder it would have read. The scan is
a request like every other read: `State::opening` asks once, the shell runs
`bedrock::scan` off-thread, and the page draws the answer.

The design constraint stands and is why there is nothing else: this is a **shelf
beside the Java shelf**, not a second launcher. No download, no version picker, no
write. `crate::open` grew its third allowed scheme (`ms-windows-store`) for the
Store button, and only for the launcher's own constant.

What is **not** proven: no Windows machine has drawn the page, so the installed
state has only been exercised through `bedrock`'s own unit tests against temp
folders. The rail's sixth slot has no reference capture to compare with -- the
reference has five -- so what is checked is that the slot is a route and that
every page still draws in every theme. See section 6 for the Windows checklist.

The design constraint worth restating: this is a **shelf beside the Java
shelf**, not a second launcher. No download, no version picker, no write.

### 3c. A self-update checks its download and does nothing else

`crates/palantir-desktop/src/update.rs` now carries the half of an updater that is
files and arithmetic: `Offer` (version/url/sha256/size), `offer_problem` (four
refusals -- not newer, not https, no usable digest, no size), `verify_download`
(size first, then `palantir_net::download`'s streaming digest) and
`install_after_verify` (verify, *then* rename, so a bad download cannot replace
anything). Six tests hold it; `NOTES.md` has the record.

What is still owed, and none of it is testable here: a signed manifest with a key
compiled into the binary (covering version, URL and digest together), the fetch
itself, running the installer with this launcher closed, rollback, and the
installer's own rename -- `sift.nsi` installs `Sift Launcher.exe` from a payload
called `PalantirMC.exe`, so an updater that looks for the payload's name replaces
nothing.

### 3d. The animation has never rendered

`installer/anim/sift.avi` is valid (ffprobe reads 96×96, 24 frames, 25 fps,
uncompressed) and `SysAnimate32` with `ACM_OPEN`/`ACM_PLAY` is the documented
way to play it from NSIS, but **no Windows host has been available here**. The
first session with one should: run the setup exe, look at the brand page, and
confirm the arcs turn rather than render as a still or a black rectangle. If it
fails, the fallback is `MUI_PAGE_WELCOME`'s bitmap (`MUI_WELCOMEFINISHPAGE_BITMAP`)
rather than a codec dependency.

Also outstanding, and lower value:

- `installer/sift.ico` and `crates/palantir-desktop/assets/brand/app.ico` have one
generator (2026-10-25): `tools/gen_brand.py`
  writes it and `crates/palantir-desktop/assets/brand/app.ico` from the brand art,
  seven BMP entries, with `--check` like the animation's. Both files are the same
  bytes, because NSIS resolves relative paths against its own directory. The exe's
  own icon and version block come from `build.rs` (`winresource`), and that path
  is Windows-only -- it compiled first on CI's exe jobs and has not been looked at
  on a desktop, which is the check still owed.
- **There is no About page, and that is decided** (2026-10-24): the reference has
  none either, so the version and the notices live in the footer the reference's
  own settings modal draws (`mt-auto text-secondary text-sm`) -- two lines, gated
  in `shell.rs`. Attribution is a shipping condition and that footer is the
  surface it uses; do not grow it into a dialog without the same reasoning.
- The payload is still `PalantirMC.exe`; the installer renames it to
  `Sift Launcher.exe` at install time (`File /oname=`), while the build still
  produces the old exe name. The rename in the product itself is untouched, and
  is a separate change from the installer's.

## 4. This machine, and what it can and cannot prove

Linux 6.8, no Windows host, no Chrome, no GPU: the Xvfb window drivers in
`tools/` are how the UI is exercised. Present and used here:

```
makensis v3.09-4          # installer compiles; cannot run
ffprobe 6.1.1             # inspection of the AVI, not generation
Python 3.12.3 + Pillow 10.2.0   # tools/gen_installer_anim.py
cargo/rustc 1.99.0 (2026-08-27)
```

Anything that requires launching the described program — the setup wizard, the
launcher's own window on Windows — is **unverifiable here**. Say so rather than
inferring it from a green compile.

## 5. The Windows checklist, and two of it are done

Every item below needs a Windows machine with the built exe or the installer on
it. **Two are ticked** — on 2026-10-24, on the owner's own Windows PC, from CI run
`40115604939`'s `palantirmc-x86_64-pc-windows-msvc` artifact, exe `sha256`
`355dafe354ed3bf1b2f68adb56f3b3c42aad22ed88d8d87c7a7e0f7f56293645` (28,648,960
bytes), installed by hand at `%LOCALAPPDATA%\Programs\Sift Launcher\` — the layout
`sift.nsi` produces, because the setup exe itself has still never been built or
run. A session that ticks another should say which machine, which build and what
it saw. Each item is written so it can be followed without reading the source.

Run the build first: take the exe from CI's `palantirmc-x86_64-pc-windows-msvc`
artifact (the run on `master`), and the setup exe from `makensis
-DVERSION=0.1.0 installer\sift.nsi` run on a Windows box with NSIS installed.

- [x] **The exe's file properties.** Right-click the exe → Properties → Details.
  Expect *Product name* `PalantirMC`, *File version* `0.1.0`, *Product version*
  `0.1.0`, the copyright line from `LICENSE`, and the mark as the file's icon in
  Explorer's list, the taskbar and Alt-Tab, at 16, 32, 48 and 256 pixels.
  **Seen 2026-10-24:** reading the version block back off the downloaded artifact
  gave `ProductName: PalantirMC`, `FileVersion: 0.1.0`, `ProductVersion: 0.1.0`,
  `CompanyName: Palantir Studios`, `LegalCopyright: Copyright (c) 2026 Palantir
  Studios. All rights reserved.` The *icon* at the four sizes is the part of this
  item nobody has looked at yet: it wants Explorer in each view mode and Alt-Tab
  with the window open.
- [x] **The launcher starts, and only one does.** Double-click it twice within a
  few seconds. Expect one window, and on the second launch a message box saying
  the launcher is already running and naming a process id. Then: open Task
  Manager, end the first launcher, delete nothing, and double-click again —
  expect it to start, because the lock file is a stale claim rather than a
  permanent one. After closing it cleanly, `%APPDATA%\PalantirMC\palantir.pid`
  (or wherever `PALANTIRMC_HOME` points) should be gone.
  **Seen 2026-10-24:** the first start draws a window titled `PalantirMC`,
  responding, and writes `%APPDATA%\PalantirMC\palantir.pid` holding its pid
  (`12724` that run) beside `palantirmc-desktop.json`. A second start within
  seconds owns a window of its own titled `PalantirMC` -- the message box -- and
  stays alive until that box is answered (it was dismissed by hand), then exits
  **0** with the first window untouched and still the only launcher. Starting a
  third time behaves the same way. The *stale-lock* half -- kill the first from
  Task Manager and start again -- has not been tried.
- [ ] **The machine stays awake through an install.** On a laptop, set a one-minute
  sleep timeout (`powercfg /change standby-timeout-ac 1`, restore it afterwards),
  then start a large install or a first launch that downloads. While it runs,
  `powercfg /requests` should list a SYSTEM request owned by the launcher. Expect
  no suspend, and the download to finish.
- [ ] **CurseForge reads, and says so when it cannot.** Put a key in
  `palantirmc-desktop.json` (`curseforge_api_key`) or set
  `PALANTIRMC_CURSEFORGE_API_KEY`, open Discover, switch *Source* to CurseForge on
  the Mods tab. Expect results whose cards carry one stat (downloads) and no
  followers. Without a key, expect the sentence naming both places a key can go
  rather than an empty list. Pressing a card should open
  `curseforge.com/projects/<id>` in the browser. Switching to the Data packs or
  Servers tab should return the source to Modrinth.
- [ ] **The Bedrock shelf.** With Minecraft Bedrock installed and played once, open
  the sixth rail slot. Expect the presence card with the game folder's path, the
  worlds listed by name, and both pack shelves filled. On a machine without it,
  expect the *not installed* card and that *Open the Store* opens the Store page
  for the game. A machine where it has never been played should show *Installed*
  and three empty sentences naming their folders.
- [ ] **The settings footer.** Open Settings. Expect `PalantirMC 0.1.0` at the
  bottom of the tab column, and `Notices travel beside the launcher:
  THIRD_PARTY_NOTICES.md` under it.
- [ ] **The installer.** Run the setup exe. Expect: the brand page plays the
  animation and shows the mark as the wizard's icon; the licence page shows
  `LICENSE`; the install lands in `%LOCALAPPDATA%\Programs\Sift Launcher` with
  `Sift Launcher.exe`, `sift.ico`, `LICENSE` and `THIRD_PARTY_NOTICES.md`; a
  desktop shortcut when that box is ticked; an entry in *Apps & features*; and
  that the finish page starts the launcher. Then uninstall, and expect the folder,
  the shortcut and the registry entry gone, with instance data untouched.
- [ ] **A screenshot of each of the above**, under `.scratch/verify/windows/`,
  named for the item — the captures are what the next session can check against.

## 6. Leftovers in the working tree, deliberately not committed

- Twelve root-level `*.png` captures from the 2026-10-17 window drivers, and
  root `.local/` (scratch). `AGENTS.md` §"Never commit build output or captures"
  covers both; they belong under `.scratch/` (already ignored) or nowhere. Not
  deleted here because they are not this session's to throw away.
- `dist/` holds CI artifacts and the dummy-payload setup exe. Ignored.

If a later session wants the tree clean, `git status` should show only those.

## 7. The egui migration, and what it still owes

Started 2026-10-25 at the owner's instruction ("just switch to egui fastframe"), and
started with a measurable slice rather than a rewrite:

- `crates/palantir-egui` — **its own workspace** (the root one cannot hold `iced_winit`
  0.12's `web-sys =0.3.67` and `eframe` 0.36's `^0.3.103` at once). One page's shape:
  300 cards, real `fastframe-scroll`, the same `PALANTIRMC_FRAME_LOG` format the iced
  shell writes.
- `vendor/fastframe-scroll` — the crate at tag `v0.6.0`, MIT, unmodified source, its
  own `UPSTREAM.md` with the blob hashes. It is not on crates.io.
- CI job `egui-slice` builds and tests it on Windows and uploads
  `palantir-egui-slice`.

**Owed, in order:**

1. **The comparison.** Run the slice and the iced launcher on the same machine with
   the frame log on, scroll both, and compare p95. Nothing else should be done until
   that number exists — it is the whole justification for the migration.
2. **The slice's lock file** is not committed; the CI job therefore builds without
   `--locked`. Commit it once the comparison says the stack is worth keeping.
3. **Pages, one at a time**, if it is: `discover` first (it is the heaviest and the
   one the reader scrolls), then `home`, then the shell chrome. Each needs the theme
   (`tools/gen_theme.py` emits iced types today), the icons (`icons_gen` emits
   `iced::widget::canvas` paths, but the path data itself is portable) and the text
   tables (`text_gen` is strings; it carries over).
4. **`Store` has to leave the binary** to be shared: it lives in
   `palantir-desktop/src/store.rs` today, and the egui crate cannot depend on a
   binary crate. That is the first real refactor of the migration and it should be
   done before the second page, not during it.

**Correction, after sizing it: `Store` is not a seam yet.** The plan above said "move
`Store` into a library both can use". Measured before starting it: `store.rs` reaches
into eighteen desktop modules -- `page`, `pages`, `ui`, `route`, `color_theme`,
`scale`, `instances`, `launch`, `meta`, `mods`, `avatar`, `skin`, `wire` and more --
and 119 call sites across the crate name `crate::store` or `crate::prefs`. It is not a
data layer with a UI on top; the UI types are inside it. Extracting it is its own
project, not a step of this one.

So the first page does not wait for it: **egui's Discover reads `palantir-net`
directly** -- the crate that is already UI-free and holds the search URL builder, the
`ModrinthSearchHit` parser and `BlockingHttpFetcher`. One page talking to a library is
a smaller lie than a four-thousand-line extraction done to avoid it, and it puts real
results on screen in the stack that scrolls at 18 ms p95. The `Store` extraction
becomes what it is: a separate slice that both stacks will want, to be planned on its
own numbers rather than as a prerequisite.
