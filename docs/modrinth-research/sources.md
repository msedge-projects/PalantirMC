# Sources, versions, licences

Everything this mission studied, and the rules that govern what may be taken
from it.

## The binary studied

| Field | Value |
| --- | --- |
| Product | Modrinth App (codename *Theseus*) — Tauri + Vue 3 desktop launcher |
| Version | **0.21.9** |
| Release tag | `v0.21.9` in `modrinth/code` |
| Asset | `Modrinth.App_0.21.9_amd64.deb` (31,119,128 bytes) |
| Asset URL | <https://github.com/modrinth/code/releases/download/v0.21.9/Modrinth.App_0.21.9_amd64.deb> |
| SHA-256 | `7c9fa6d060770d8b206a5714693c106343fe69fd7a62afd707a80f7e88198e26` |
| Release published | 2026-10-07T20:00:56Z |
| Installed | 2026-10-14 |
| Displayed version confirmed in-app | see `MANIFEST.md` (About dialog, `S-0xxx`) |

Downloaded from the project's own GitHub releases — the official channel the
mission permits as the fallback. `modrinth.com/app` is a JS-rendered page whose
HTML contains no asset URL; the release API was used instead. No unofficial
mirror was used at any point.

Other assets published for 0.21.9 and deliberately **not** installed, because one
per-platform binary is enough to study: `.rpm`, `.dmg`, `x64-setup.exe`.

## The source studied

| Field | Value |
| --- | --- |
| Repository | `modrinth/code` — <https://github.com/modrinth/code> |
| Clone | `/tmp/modrinth-code` (shallow, sparse: `apps/`, `packages/`) |
| Commit | `6ebd75b68dd386cd4d74eee1cf2e96d945c937db` |
| Commit date | 2026-10-07T12:36:06-07:00 |
| Pinned to | tag `v0.21.9` — **the same build as the binary above** |
| Retrieved | 2026-10-14 |

Source and binary are pinned to one tag, so §10's "app version drift" branch is
**not** in play for this mission: every `SOURCE` citation is against exactly the
code that produced the running UI. (Before pinning, `main` was 6 days ahead of
the release at `8b1d9216`; the pin was made on purpose, not after a
disagreement appeared.)

Relevant subtrees: `apps/app-frontend` (Vue UI), `apps/app` (Tauri shell +
Rust commands), `packages/app-lib` (Rust plugin/logic layer), `packages/api`,
`packages/assets` (branding), `apps/labrinth` (the API server).

`/tmp/modrinth-code` is ground truth and **never enters our repository trees**.
It is scratch; a re-clone is cheap.

## Ancillary sources (`SOURCE-web`)

| Source | URL | Retrieved | Used for |
| --- | --- | --- | --- |
| Theseus development guide | <https://docs.modrinth.com/contributing/theseus/> | 2026-10-14 | architecture orientation, build layout |
| Modrinth changelog | <https://modrinth.com/news/changelog> | 2026-10-14 | date-stamping behaviour changes |
| Modrinth support/help articles | <https://support.modrinth.com/> | 2026-10-14 | user-facing explanations of flows |
| Modrinth App download page | <https://modrinth.com/app> | 2026-10-14 | version confirmation (page is JS-rendered) |

Every `SOURCE-web` claim in the modules carries its URL and the 2026-10-14
retrieval date inline.

## Licences and branding — what we may and may not take

`modrinth/code` is a monorepo; **the licence is per-package**, and the
monorepo carries its own copying guidelines that restrict code *and*
branding. Read from the checkout's own `COPYING.md` and the root `README.md`:

| Package | Licence | Practical consequence for this mission |
| --- | --- | --- |
| `packages/assets` (branding) | **All rights reserved** — Modrinth's branding pack | Logo, wordmark, app icon, colour tokens, marketing art. **No copying, including into private trees.** We use the mark only as identification in prose. |
| `apps/app-frontend`, `apps/app` | GPL-3.0 | Copying source would impose GPL on our work. This is why the mission is clean-room: we read behaviour, we never copy code. |
| `packages/app-lib`, `packages/api`, most `packages/*` | GPL-3.0 | Same. |
| Community-authored translations (Crowdin) | Varies by contributor | Translation files are **not** copied; i18n *approach* is documented, not its strings. |

Consequences, applied throughout this tree:

- **No upstream source, CSS, asset, icon, image or branding is copied into
  `/PalantirMC`.** Verified by construction — every file here is prose, a
  screenshot, or our own script.
- Quoting a short UI label ("Create an instance", "Getting started") in a
  research document is identification, is cited, and is fine.
- Translation files, icon sets and image assets are not reproduced.
- The app's green `#1bd96a`-family accents are noted as *facts about a rival's
  design*, not as a palette to copy; our own copper/matte language stands.

## Screenshots — internal reference only

Every `screenshots/S-*.png` in this tree is **internal reference material for
our implementer**. They are not assets to ship, not product screenshots, and not
marketing material. They capture a third party's UI under its own trademarks.
They exist so the implementing agent can see behaviour and information
architecture without opening the Modrinth App again — which is the whole point
of the mission. Do not paste them into our UI, our docs site, or our store
listings, and do not ship them inside the binary.

## Capture environment — how the screenshots were produced

The app was driven on a **private Xvfb display** (`:201`), never the owner's
`:0`. Captures are made by screenshotting the window, OCR-ing it, and **clicking
only controls that were read on screen** (`tools/mrcap.py`, and the phrase-aware
`tools/mrdrive.py`). Nothing was clicked at an assumed coordinate; the two
exceptions are green-on-green button labels OCR cannot read, which were clicked at
a position read off a captured frame and logged.

The studied binary is **unmodified**. The only environment variable of note is
`GIO_USE_NETWORK_MONITOR=base`, which selects which connectivity implementation
the GTK stack reports to the webview; without it the app's `navigator.onLine` is
falsely `false`, which disables its own create-instance action. This is an
environment fact, not a patch — see OQ-004 and OQ-012. No account or credential of
any kind was created or used at any point.

**Two capture modes are on record.** Most frames were taken under a **bare Xvfb**
(no window manager), where the app's own window controls are inert and maximize is
a no-op. For the window-control, close-behaviour and native-decoration frames
(`S-0134`, `S-0149`, `S-0150`) a lightweight window manager, **openbox**, was also
run on `:201` (`systemd-run --unit=ob201 … openbox`); only with a WM do those
controls do anything, and only a whole-screen capture then includes the WM-drawn
frame. The **genuine-offline** frames (`S-0151`, `S-0152`) were taken with the app
started inside `unshare -n` (its own empty network namespace) and with the default
network monitor, so the app really had no network. Everything else — the binary,
the pinned source, the data — is identical across modes.