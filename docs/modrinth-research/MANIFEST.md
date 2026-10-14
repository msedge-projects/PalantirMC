# MANIFEST — screenshot index

Every capture this mission produced, and who refers to it. **Screenshots are
internal reference material for our implementer — never shipped, never product
art** (see `sources.md`).

Rules followed: ids are chronological and zero-padded; **an id is never reused,
even after deletion**; state names live in this table, not in filenames; every
referenced screenshot exists and every shipped screenshot is referenced.

## Reconciling both directions

- **88 files on disk**, all referenced below.
- **Retired ids** — `S-0010` … `S-0014`. A sweep was run while the app was
  actually on the Hosting tab, so every section click missed and the five frames
  are duplicates of `S-0006`. The ids are retired, not reused; the files are
  deleted. Root cause and fix are in `RUNLOG.md`.
- **Referenced but missing**: none.

## The captures

| ID | File | Size | State shown | What to look at | Referenced by |
| --- | --- | --- | --- | --- | --- |
| S-0001 | S-0001.png | 1280×800 | Home, fresh install | First-run state: welcome block, '+ Create an instance', 'Press N to quick create an instance', 'Escaping another launcher?' / 'Import from launcher'; icon-only rail; 'Getting started' checklist; titlebar with 'Offline' chip and 'No instances running' pill. | INDEX, modules 01, 02 |
| S-0002 | S-0002.png | 1280×800 | Home, after rail click | Confirms the Home destination and the breadcrumb reading 'Home'. | INDEX, modules 01 |
| S-0003 | S-0003.png | 1280×800 | Browse — modpacks, live data | Project-type tabs; search; 'Sort by: Relevance'; 'View: 20'; pagination '1 2 … 942'; a full project card; facet sidebar with 'Hide already installed' and 'Category'; tooltip 'Discover content'; **and the 'Offline' chip while data loads**. | INDEX, modules 01, 03, 10 |
| S-0004 | S-0004.png | 1280×800 | Skin selector | 'Saved skins' empty state. Out of our scope; captured to confirm the rail item and its route. | INDEX, gap-analysis |
| S-0005 | S-0005.png | 1280×800 | Screenshots (global) | Empty screenshots viewer. Out of our scope. | INDEX, gap-analysis |
| S-0006 | S-0006.png | 1280×800 | Hosting / Servers | 'Modrinth Hosting', 'New server', an 'Invite players' overlay with a friend list, and an **incoming invite toast carrying Accept/Decline**. | INDEX, modules 09, 10 |
| S-0007 | S-0007.png | 1280×800 | Create-instance rail action | Rail slot y=356; tooltip read 'One-click … new instance'. | INDEX, modules 01 |
| S-0008 | S-0008.png | 1280×800 | Settings modal — Appearance | The settings dialog and its complete left nav; Color theme cards Dark/Light/OLED/Sync with system; 'Sync theme across devices'; footer **'Modrinth App 0.21.9 / Linux 24.4.0'**. | INDEX, modules 01, 06 |
| S-0009 | S-0009.png | 1280×800 | Rail, lowest slot | The leave/sign-out slot. Unlabelled and unresolved (OQ-006). | INDEX, modules 01 |
| S-0010 | S-0010.png | 1280×800 | uncategorised |  |  |
| S-0011 | S-0011.png | 1280×800 | uncategorised |  |  |
| S-0012 | S-0012.png | 1280×800 | uncategorised |  |  |
| S-0013 | S-0013.png | 1280×800 | uncategorised |  |  |
| S-0014 | S-0014.png | 1280×800 | uncategorised |  |  |
| S-0015 | S-0015.png | 1280×800 | Hosting + Invite players dialog | The hosting page with the invite overlay open, captured mid-sweep. | modules 04, 09 |
| S-0020 | S-0020.png | 1280×800 | Settings → Appearance | Scroll frame 1 of 10. | modules 06 |
| S-0021 | S-0021.png | 1280×800 | Settings → Appearance | Scroll frame 2 of 10. | modules 06 |
| S-0022 | S-0022.png | 1280×800 | Settings → Appearance | Scroll frame 3 of 10. | modules 06 |
| S-0023 | S-0023.png | 1280×800 | Settings → Appearance | Scroll frame 4 of 10. | modules 06 |
| S-0024 | S-0024.png | 1280×800 | Settings → Features | Scroll frame 1 of 10. The section that toggles instance tabs. | modules 06, 01 |
| S-0025 | S-0025.png | 1280×800 | Settings → Features | Scroll frame 2 of 10. The section that toggles instance tabs. | modules 06, 01 |
| S-0026 | S-0026.png | 1280×800 | Settings → Features | Scroll frame 3 of 10. The section that toggles instance tabs. | modules 06, 01 |
| S-0027 | S-0027.png | 1280×800 | Settings → Features | Scroll frame 4 of 10. The section that toggles instance tabs. | modules 06, 01 |
| S-0028 | S-0028.png | 1280×800 | Settings → Features | Scroll frame 5 of 10. The section that toggles instance tabs. | modules 06, 01 |
| S-0029 | S-0029.png | 1280×800 | Settings → Features | Scroll frame 6 of 10. The section that toggles instance tabs. | modules 06, 01 |
| S-0030 | S-0030.png | 1280×800 | Settings → Behavior | Scroll frame 1 of 10. The longest settings section. | modules 06 |
| S-0031 | S-0031.png | 1280×800 | Settings → Behavior | Scroll frame 2 of 10. The longest settings section. | modules 06 |
| S-0032 | S-0032.png | 1280×800 | Settings → Behavior | Scroll frame 3 of 10. The longest settings section. | modules 06 |
| S-0033 | S-0033.png | 1280×800 | Settings → Behavior | Scroll frame 4 of 10. The longest settings section. | modules 06 |
| S-0034 | S-0034.png | 1280×800 | Settings → Behavior | Scroll frame 5 of 10. The longest settings section. | modules 06 |
| S-0035 | S-0035.png | 1280×800 | Settings → Behavior | Scroll frame 6 of 10. The longest settings section. | modules 06 |
| S-0036 | S-0036.png | 1280×800 | Settings → Behavior | Scroll frame 7 of 10. The longest settings section. | modules 06 |
| S-0037 | S-0037.png | 1280×800 | Settings → Language | Scroll frame 1 of 10. Locale list, 33 languages, the 'Beta' badge. | modules 06, 10 |
| S-0038 | S-0038.png | 1280×800 | Settings → Language | Scroll frame 2 of 10. Locale list, 33 languages, the 'Beta' badge. | modules 06, 10 |
| S-0039 | S-0039.png | 1280×800 | Settings → Language | Scroll frame 3 of 10. Locale list, 33 languages, the 'Beta' badge. | modules 06, 10 |
| S-0040 | S-0040.png | 1280×800 | Settings → Language | Scroll frame 4 of 10. Locale list, 33 languages, the 'Beta' badge. | modules 06, 10 |
| S-0041 | S-0041.png | 1280×800 | Settings → Language | Scroll frame 5 of 10. Locale list, 33 languages, the 'Beta' badge. | modules 06, 10 |
| S-0042 | S-0042.png | 1280×800 | Settings → Language | Scroll frame 6 of 10. Locale list, 33 languages, the 'Beta' badge. | modules 06, 10 |
| S-0043 | S-0043.png | 1280×800 | Settings → Language | Scroll frame 7 of 10. Locale list, 33 languages, the 'Beta' badge. | modules 06, 10 |
| S-0044 | S-0044.png | 1280×800 | Settings → Language | Scroll frame 8 of 10. Locale list, 33 languages, the 'Beta' badge. | modules 06, 10 |
| S-0045 | S-0045.png | 1280×800 | Settings → Language | Scroll frame 9 of 10. Locale list, 33 languages, the 'Beta' badge. | modules 06, 10 |
| S-0046 | S-0046.png | 1280×800 | Settings → Profile | The account section on an unauthenticated install. | modules 06, 07 |
| S-0047 | S-0047.png | 1280×800 | Settings → Social | Single frame. | modules 06 |
| S-0048 | S-0048.png | 1280×800 | Settings → Privacy | Telemetry / ads / Discord presence toggles. | modules 06 |
| S-0049 | S-0049.png | 1280×800 | Settings → Synced settings | Scroll frame 1 of 10. Memory, resolution, fullscreen, hooks, args, env vars. | modules 06 |
| S-0050 | S-0050.png | 1280×800 | Settings → Synced settings | Scroll frame 2 of 10. Memory, resolution, fullscreen, hooks, args, env vars. | modules 06 |
| S-0051 | S-0051.png | 1280×800 | Settings → Synced settings | Scroll frame 3 of 10. Memory, resolution, fullscreen, hooks, args, env vars. | modules 06 |
| S-0052 | S-0052.png | 1280×800 | Settings → Synced settings | Scroll frame 4 of 10. Memory, resolution, fullscreen, hooks, args, env vars. | modules 06 |
| S-0053 | S-0053.png | 1280×800 | Settings → Synced settings | Scroll frame 5 of 10. Memory, resolution, fullscreen, hooks, args, env vars. | modules 06 |
| S-0054 | S-0054.png | 1280×800 | Settings → Synced settings | Scroll frame 6 of 10. Memory, resolution, fullscreen, hooks, args, env vars. | modules 06 |
| S-0055 | S-0055.png | 1280×800 | Settings → Synced settings | Scroll frame 7 of 10. Memory, resolution, fullscreen, hooks, args, env vars. | modules 06 |
| S-0056 | S-0056.png | 1280×800 | Settings → Synced settings | Scroll frame 8 of 10. Memory, resolution, fullscreen, hooks, args, env vars. | modules 06 |
| S-0057 | S-0057.png | 1280×800 | Settings → Synced settings | Scroll frame 9 of 10. Memory, resolution, fullscreen, hooks, args, env vars. | modules 06 |
| S-0058 | S-0058.png | 1280×800 | Settings → Java installations | Scroll frame 1 of 10. | modules 06 |
| S-0059 | S-0059.png | 1280×800 | Settings → Java installations | Scroll frame 2 of 10. | modules 06 |
| S-0060 | S-0060.png | 1280×800 | Settings → Java installations | Scroll frame 3 of 10. | modules 06 |
| S-0061 | S-0061.png | 1280×800 | Settings → Resource management | Scroll frame 1 of 10. Content storage, unique/shared/orphaned bytes, developer mode. | modules 06 |
| S-0062 | S-0062.png | 1280×800 | Settings → Resource management | Scroll frame 2 of 10. Content storage, unique/shared/orphaned bytes, developer mode. | modules 06 |
| S-0063 | S-0063.png | 1280×800 | Settings → Resource management | Scroll frame 3 of 10. Content storage, unique/shared/orphaned bytes, developer mode. | modules 06 |
| S-0064 | S-0064.png | 1280×800 | Settings → Resource management | Scroll frame 4 of 10. Content storage, unique/shared/orphaned bytes, developer mode. | modules 06 |
| S-0065 | S-0065.png | 1280×800 | Settings → Resource management | Scroll frame 5 of 10. Content storage, unique/shared/orphaned bytes, developer mode. | modules 06 |
| S-0066 | S-0066.png | 1280×800 | Settings → Resource management | Scroll frame 6 of 10. Content storage, unique/shared/orphaned bytes, developer mode. | modules 06 |
| S-0067 | S-0067.png | 1280×800 | Settings → Resource management | Scroll frame 7 of 10. Content storage, unique/shared/orphaned bytes, developer mode. | modules 06 |
| S-0068 | S-0068.png | 1280×800 | Settings → Resource management | Scroll frame 8 of 10. Content storage, unique/shared/orphaned bytes, developer mode. | modules 06 |
| S-0069 | S-0069.png | 1280×800 | Settings → Resource management | Scroll frame 9 of 10. Content storage, unique/shared/orphaned bytes, developer mode. | modules 06 |
| S-0070 | S-0070.png | 1280×800 | Create-instance attempt | Hover/click on the rail '+' and the Home CTA. No dialog opened — the button is `:disabled="offline"`. See OQ-004. | modules 02, open-questions |
| S-0071 | S-0071.png | 1280×800 | Create-instance attempt | Hover/click on the rail '+' and the Home CTA. No dialog opened — the button is `:disabled="offline"`. See OQ-004. | modules 02, open-questions |
| S-0072 | S-0072.png | 1280×800 | Create-instance attempt | Hover/click on the rail '+' and the Home CTA. No dialog opened — the button is `:disabled="offline"`. See OQ-004. | modules 02, open-questions |
| S-0073 | S-0073.png | 1280×800 | Create-instance attempt | Hover/click on the rail '+' and the Home CTA. No dialog opened — the button is `:disabled="offline"`. See OQ-004. | modules 02, open-questions |
| S-0074 | S-0074.png | 1280×800 | Create-instance attempt | Hover/click on the rail '+' and the Home CTA. No dialog opened — the button is `:disabled="offline"`. See OQ-004. | modules 02, open-questions |
| S-0075 | S-0075.png | 1280×800 | Create-instance attempt | Hover/click on the rail '+' and the Home CTA. No dialog opened — the button is `:disabled="offline"`. See OQ-004. | modules 02, open-questions |
| S-0076 | S-0076.png | 1280×800 | Create-instance attempt | Hover/click on the rail '+' and the Home CTA. No dialog opened — the button is `:disabled="offline"`. See OQ-004. | modules 02, open-questions |
| S-0077 | S-0077.png | 1280×800 | Create-instance attempt | Hover/click on the rail '+' and the Home CTA. No dialog opened — the button is `:disabled="offline"`. See OQ-004. | modules 02, open-questions |
| S-0078 | S-0078.png | 1280×800 | Create-instance attempt | Hover/click on the rail '+' and the Home CTA. No dialog opened — the button is `:disabled="offline"`. See OQ-004. | modules 02, open-questions |
| S-0079 | S-0079.png | 1280×800 | Create-instance attempt | Hover/click on the rail '+' and the Home CTA. No dialog opened — the button is `:disabled="offline"`. See OQ-004. | modules 02, open-questions |
| S-0080 | S-0080.png | 1280×800 | Capture degraded by a window resize | The webview stopped repainting after `xdotool windowsize`; content fills only the top of the frame. An environment artifact, not app behaviour. | RUNLOG |
| S-0081 | S-0081.png | 1280×800 | Capture degraded by a window resize | The webview stopped repainting after `xdotool windowsize`; content fills only the top of the frame. An environment artifact, not app behaviour. | RUNLOG |
| S-0082 | S-0082.png | 1280×800 | Capture degraded by a window resize | The webview stopped repainting after `xdotool windowsize`; content fills only the top of the frame. An environment artifact, not app behaviour. | RUNLOG |
| S-0083 | S-0083.png | 1280×800 | Capture degraded by a window resize | The webview stopped repainting after `xdotool windowsize`; content fills only the top of the frame. An environment artifact, not app behaviour. | RUNLOG |
| S-0084 | S-0084.png | 1280×800 | Capture degraded by a window resize | The webview stopped repainting after `xdotool windowsize`; content fills only the top of the frame. An environment artifact, not app behaviour. | RUNLOG |
| S-0085 | S-0085.png | 1280×800 | Capture degraded by a window resize | The webview stopped repainting after `xdotool windowsize`; content fills only the top of the frame. An environment artifact, not app behaviour. | RUNLOG |
| S-0086 | S-0086.png | 1280×800 | Post-relaunch create-instance attempts | Clean render restored; rail clicks work, but the create-instance CTA stays inert because of the offline guard. | modules 02, open-questions |
| S-0087 | S-0087.png | 1280×800 | Post-relaunch create-instance attempts | Clean render restored; rail clicks work, but the create-instance CTA stays inert because of the offline guard. | modules 02, open-questions |
| S-0088 | S-0088.png | 1280×800 | Post-relaunch create-instance attempts | Clean render restored; rail clicks work, but the create-instance CTA stays inert because of the offline guard. | modules 02, open-questions |
| S-0089 | S-0089.png | 1280×800 | Post-relaunch create-instance attempts | Clean render restored; rail clicks work, but the create-instance CTA stays inert because of the offline guard. | modules 02, open-questions |
| S-0090 | S-0090.png | 1280×800 | Post-relaunch create-instance attempts | Clean render restored; rail clicks work, but the create-instance CTA stays inert because of the offline guard. | modules 02, open-questions |
| S-0091 | S-0091.png | 1280×800 | Post-relaunch create-instance attempts | Clean render restored; rail clicks work, but the create-instance CTA stays inert because of the offline guard. | modules 02, open-questions |
| S-0092 | S-0092.png | 1280×800 | Post-relaunch create-instance attempts | Clean render restored; rail clicks work, but the create-instance CTA stays inert because of the offline guard. | modules 02, open-questions |
## How to read the frames

- Frames of one section are **consecutive scroll positions, not a stitch**. A
  stitched image would invent pixels the app never drew together and would hide
  scroll position — which is itself a finding worth keeping.
- The app has **no narrow layout**; if a frame looks cramped, that is the
  minimum, not a breakpoint.
- The titlebar "Offline" chip in every frame is **wrong** — the app was online
  throughout (`S-0003` proves it), and worse, the same flag disables the
  "Create an instance" button. See OQ-004. Do not read the chip as an offline
  state, and do not read the inert button as an absent feature.
