# MANIFEST — screenshot index

Every capture the mission produced, and who refers to it. **Screenshots are
internal reference material for our implementer — never shipped, never product
art** (see `sources.md`).

Rules followed: ids are chronological and zero-padded; **an id is never reused,
even after deletion**; state names live in this table, not in filenames; every
referenced screenshot exists and every shipped screenshot is referenced.

Window size is 1280×800 for all frames — the app's default. The mission asked
for 840×600 as well, and that is **impossible**: `apps/app/tauri.conf.json`
declares `minWidth: 1100, minHeight: 700` (OQ-002). The narrow end is documented
by capture below, not by a squeeze.

## Reconciling both directions

- **Retired ids** — `S-0010` … `S-0014`. A sweep was run while the app was
  actually on the Hosting tab, so every section click missed and the five frames
  are duplicates of `S-0006`. The ids are retired, not reused; the files are
  deleted. Root cause and fix are in `RUNLOG.md`.
- **Captures with no section**: none — every live frame is referenced below.
- **Missing captures**, listed so the gaps are visible rather than implied: the
  add-instance dialog, the install flow with dependencies, a populated library
  (the research account has no instances, and creating one needs a Minecraft
  account, which mission §1.4 forbids), launch/runtime states, and genuine
  offline states.

## The captures

| ID | File | Size | State shown | What to look at | Referenced by |
| --- | --- | --- | --- | --- | --- |
| S-0001 | S-0001.png | 1280×800 | Home, fresh install | First-run state: "Welcome to Modrinth", "+ Create an instance", "Press N to quick create an instance", "Escaping another launcher?" / "Import from launcher"; icon-only left rail; right sidebar "Getting started" checklist + "News" + Hosting promo; titlebar with "Offline" chip and "No instances running" pill | INDEX, modules 01, 02 |
| S-0002 | S-0002.png | 1280×800 | Home, after rail click | Confirms the Home destination and the breadcrumb reading "Home"; "Import from launcher" | INDEX, modules 01 |
| S-0003 | S-0003.png | 1280×800 | Browse — modpacks, **live data** | Project-type tabs (Modpacks/Mods/Resource Packs/Data Packs/Shaders/Servers); search field; "Sort by: Relevance"; "View: 20"; pagination `1 2 … 942`; a project card in full; facet sidebar with "Hide already installed" and "Category"; tooltip "Discover content"; **and the "Offline" chip while data loads** | INDEX, modules 03, 01, 10 |
| S-0004 | S-0004.png | 1280×800 | Skin selector | `"" Saved skins` empty state. Out of our scope; captured to confirm the rail item and its route | INDEX, gap-analysis |
| S-0005 | S-0005.png | 1280×800 | Screenshots (global) | Empty screenshots viewer. Out of our scope | INDEX, gap-analysis |
| S-0006 | S-0006.png | 1280×800 | Hosting / Servers | "Modrinth Hosting", "New server", "Invite players" overlay with friend list and an **incoming invite toast carrying Accept/Decline** — the one place a toast must be actionable rather than informative | INDEX, modules 09, 10 |
| S-0007 | S-0007.png | 1280×800 | Create-instance rail action | Rail slot y=356; tooltip read "One-click … new instance". Did not transition in this frame | INDEX, modules 01 |
| S-0008 | S-0008.png | 1280×800 | **Settings modal — Appearance** | The settings dialog and its complete left nav (DISPLAY: Appearance/Features/Behavior/Language[Beta]; ACCOUNT: Profile/Social/Privacy; INSTANCES: Synced settings/Java installations/Resource management); Color theme cards Dark/Light/OLED/Sync with system; "Sync theme across devices" toggle; footer **"Modrinth App 0.21.9 / Linux 24.4.0"** | INDEX, modules 06, 01 |
| S-0009 | S-0009.png | 1280×800 | Rail, lowest slot | The leave/sign-out slot. Unlabelled and unresolved (OQ-006) | INDEX, modules 01 |
| S-0020 | S-0020.png | 1280×800 | Settings → Appearance, frame 0 | Section as loaded | modules 06 |
| S-0021 | S-0021.png | 1280×800 | Settings → Appearance, scrolled | Advanced-rendering row and its warning | modules 06 |
| S-0022 | S-0022.png | 1280×800 | Settings → Appearance, scrolled | Appearance rows below the theme cards | modules 06 |
| S-0023 | S-0023.png | 1280×800 | Settings → Appearance, end | Bottom of the Appearance section | modules 06 |
| S-0024…S-0029 | S-0024.png … | 1280×800 | Settings → **Features**, 6 scroll frames | The section that toggles instance tabs — Files/Worlds/Screenshots/skin-selector. The mission's §5 hint about these moving out of instance settings | modules 06, 01 |
| S-0030…S-0036 | S-0030.png … | 1280×800 | Settings → **Behavior**, 7 scroll frames | The longest settings section. Launch page, hide-on-start, modpack warnings, concurrency | modules 06 |
| S-0037…S-0045 | S-0037.png … | 1280×800 | Settings → **Language**, 9 scroll frames | Locale list, **33** shipped languages, and the "Beta" badge | modules 06, 10 |
| S-0046 | S-0046.png | 1280×800 | Settings → **Profile** | Single frame: the account section on an unauthenticated install | modules 06, 07 |
| S-0047 | S-0047.png | 1280×800 | Settings → **Social** | Single frame | modules 06 |
| S-0048 | S-0048.png | 1280×800 | Settings → **Privacy** | Single frame: telemetry / ads / Discord presence toggles | modules 06 |
| S-0049 | S-0049.png | 1280×800 | Settings → **Synced settings — capture failed** | The section click missed, so this frame is **not** the Synced settings section. Retained rather than deleted so the miss is visible | `RUNLOG.md` |
| S-0050 | S-0050.png | 1280×800 | Settings → **Java installations — capture failed** | Same: click missed | `RUNLOG.md` |

## How to read the frames

- Frames of one section are **consecutive scroll positions, not a stitch**. A
  stitched image would invent pixels the app never drew together and would hide
  scroll position — which is itself a finding worth keeping.
- The app has **no narrow layout**; if a frame looks cramped, that is the
  minimum, not a breakpoint.
- The titlebar "Offline" chip in every frame is **wrong** — the app was online
  throughout (`S-0003` proves it). Do not read it as evidence of an offline
  state. See OQ-004.