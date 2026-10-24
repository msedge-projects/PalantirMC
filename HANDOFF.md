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

### 3a. CurseForge is not in Discover (highest value, smallest surface)

The client and the manifest parser are ready; the page was not touched. The
seams, by name:

- `pages/discover.rs` → `Asked { round, query: palantir_net::engine::Search,
  hide_installed }`. `Search` is Modrinth's query type, so a second catalogue
  needs either a source on `Asked` or a `CfQuery` arm; a source enum is the
  smaller change and keeps `round` meaning one thing.
- `store.rs` → `Store::search(&ApiSearch)` is the single engine door.
  `Store::curseforge_search(&CfQuery)` is the sibling that landed on 2026-10-24:
  blocking, `not_implemented` when there is no engine, and uncached because
  Overwolf's terms forbid storing an answer. It resolves the key itself through
  `Store::curseforge_key()` rather than taking it as a parameter, so the shell
  calls it exactly as it calls `search` and there is one answer about where the
  key comes from.
- `shell.rs` → a `Request` variant beside the existing search request, with the
  same round-based staleness rule, plus the icon fetch the cards already use.
- **The API key has a home now** (2026-10-24). `Prefs::curseforge_api_key` is a
  row in this launcher's own preferences file, shaped like
  `Prefs::microsoft_client_id`: skipped when empty, trimmed, and overridden by
  `PALANTIRMC_CURSEFORGE_API_KEY` for one run. It is **not** a shipped key and
  **not** in an OS keychain — Overwolf issues one per developer account, and
  `accounts.json` keeps its tokens inline for Prism compatibility, so the key
  lives where the launcher's other settings live. A request without one answers
  `store::no_curseforge_key()`, which names both places a key can be.
  `NOTES.md` has the reasoning and the measured tests.

A source chip that does not work is worse than no chip: ship the switch only
with both catalogues reading.

### 3b. Bedrock is a module, not a section

`bedrock.rs` compiles and is tested by 6 cases; nothing calls it. What is
missing is entirely presentation and routing: a route/rail entry, a page that
draws `Presence` (installed vs. the Store link), the worlds list and the two
packs shelves (`PackKind::Resource` / `Behavior`), with the empty state that
says where the folder is. `crate::bedrock` is `pub` inside the binary, so a page
can call it directly; no store plumbing is needed, because every read is local
and cheap.

The design constraint worth restating: this is a **shelf beside the Java
shelf**, not a second launcher. No download, no version picker, no write.

### 3c. The animation has never rendered

`installer/anim/sift.avi` is valid (ffprobe reads 96×96, 24 frames, 25 fps,
uncompressed) and `SysAnimate32` with `ACM_OPEN`/`ACM_PLAY` is the documented
way to play it from NSIS, but **no Windows host has been available here**. The
first session with one should: run the setup exe, look at the brand page, and
confirm the arcs turn rather than render as a still or a black rectangle. If it
fails, the fallback is `MUI_PAGE_WELCOME`'s bitmap (`MUI_WELCOMEFINISHPAGE_BITMAP`)
rather than a codec dependency.

Also outstanding, and lower value:

- `installer/sift.ico` has no generator and no `--check`; it was converted once
  by an ad-hoc script from the current mark. It is a placeholder and should be
  replaced with Sift's own logo, ideally by a `tools/gen_installer_icon.py` that
  writes BMP entries and can be `--check`ed like the animation.
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

## 5. Leftovers in the working tree, deliberately not committed

- Twelve root-level `*.png` captures from the 2026-10-17 window drivers, and
  root `.local/` (scratch). `AGENTS.md` §"Never commit build output or captures"
  covers both; they belong under `.scratch/` (already ignored) or nowhere. Not
  deleted here because they are not this session's to throw away.
- `dist/` holds CI artifacts and the dummy-payload setup exe. Ignored.

If a later session wants the tree clean, `git status` should show only those.
