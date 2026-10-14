# RUNLOG — Modrinth App research mission

Append-only. One line per action. Timestamps are UTC, this host's clock.

Format: `YYYY-MM-DDTHH:MM:SSZ  <what>  <evidence / result>`

---

2026-10-14T12:47Z  Read mission prompt at `/PalantirMC/MODRINTH-RESEARCH-PROMPT.md` (408 lines). Phase 0 begins.
2026-10-14T12:48Z  Created deliverable tree `docs/modrinth-research/{modules,screenshots,tools}`.
2026-10-14T12:48Z  Cloned `modrinth/code` shallow+sparse (`apps`,`packages`) to `/tmp/modrinth-code`. Source commit first read: `8b1d9216` (main, 2026-10-13).
2026-10-14T12:49Z  Private display `:201` started via `systemd-run` (`Xvfb :201 -screen 0 1920x1080x24`). No use of `:0`.
2026-10-14T12:49Z  `modrinth.com/app` is JS-driven — no static download link in HTML. Fell back to the GitHub releases API (allowed by §0).
2026-10-14T12:49Z  Resolved latest official App release: **v0.21.9**, published 2026-10-07T20:00:56Z.
2026-10-14T12:49Z  Downloaded official asset `Modrinth.App_0.21.9_amd64.deb` from the `modrinth/code` release. SHA-256 `7c9fa6d060770d8b206a5714693c106343fe69fd7a62afd707a80f7e88198e26`.
2026-10-14T12:49Z  Installed `tesseract-ocr` (was MISSING, mission §4 assumes present).
2026-10-14T12:50Z  Extracted .deb with `dpkg -x`. Binary lands at `usr/bin/ModrinthApp`, **not** `opt/` — the first launch attempt failed on a wrong path read from an `ls` that had listed the wrong directory. Corrected, staged a durable copy at `/usr/local/bin/ModrinthApp`.
2026-10-14T12:50Z  `ldd` on the App binary: 0 missing shared libraries. No dependency install needed.
2026-10-14T12:51Z  Fetched release tag `v0.21.9` and re-pinned the source checkout to it so **source and binary are the same build** (§10 "pin both to the same release tag"). Resolved commit `6ebd75b68dd386cd4d74eee1cf2e96d945c937db` (2026-10-07T12:36:06-07:00). Checkout is 431 MB, ground truth, never enters our repo trees.
2026-10-14T12:51Z  Launched the App on `:201` with the §10 degradation env set (`GDK_BACKEND=x11`, `XDG_RUNTIME_DIR`, `GSETTINGS_BACKEND=memory`, `NO_AT_BRIDGE=1`, `GTK_A11Y=none`). **Window appeared on the first attempt** — the known GTK-under-Xvfb failure did not occur. No `Xephyr` fallback needed.
2026-10-14T12:52Z  App log shows only `libEGL warning: DRI3 error` — software rendering, cosmetic, not blocking.
2026-10-14T12:52Z  Started `openbox` on `:201` so windows have decorations, focus and resize (none was running; the first capture proved resize-by-xdotool works either way).
2026-10-14T12:53Z  Wrote `tools/mrcap.py` — observed-controls-only capture/OCR/click, reusing the `rewrite/tools/ui_smoke.py` pattern the mission points at.
2026-10-14T12:53Z  **S-0001** captured at 1280x800: Home/welcome tab, icon-only left rail (9 slots), right sidebar with "Getting started" + "News", "Offline" chip, "No instances running" pill. OCR reads the whole shell.
2026-10-14T12:54Z  Wrote `sources.md` (app version, tag, SHA-256, commit, per-package licences, branding rules, screenshot non-use note) and `open-questions.md` (OQ-001…OQ-005).
2026-10-14T12:55Z  **Single-instance finding.** A second `ModrinthApp` cannot run on this host at all, on any display: `tauri-plugin-single-instance` (`apps/app/src/main.rs:177`) binds a Linux **abstract** unix socket, which is kernel-global. Tested twice — fully isolated `HOME`/`XDG_*`, and shared `HOME`; both exit `0` immediately, no window, empty log. Binary patching rejected (identifier appears once, inside an embedded `.desktop`, not in the socket-name path). **Decision: one app instance on `:201`, driven only by the orchestrator; parallelism moved to source mining.** See OQ-001.
2026-10-14T12:55Z  Confirmed from `apps/app/tauri.conf.json`: window default 1280×800, **`minWidth: 1100`, `minHeight: 700`**, `decorations: false`, `hiddenTitle: true`, `titleBarStyle: Overlay`. The mission's requested 840×600 capture is therefore impossible — the narrow end is 1100×700 and the app has no narrow layout at all. See OQ-002.
2026-10-14T12:56Z  Wrote `tools/mrcap.py` (capture/OCR/click-by-observed-text, plus `hover` for the icon-only rail and `rail`) and `tools/mrcap_tabs.py` (hover→read tooltip→click→capture, so no slot is ever clicked blind).
2026-10-14T12:57Z  **Rail walk captured all top-level tabs.** Tooltip-first grounding paid off — the rail is icon-only, so the tooltip is what names each destination:
  - slot y=72  → **Home** (S-0002)
  - slot y=124 → **Discover content** (S-0003)
  - slot y=176 → **Skin selector** (S-0004)
  - slot y=228 → **Screenshots** (S-0005)
  - slot y=280 → **Hosting / Servers** (S-0006)
  - slot y=356 → **Create instance** ("One-click … new instance")
  - slot y=716 → **Settings** (S-0008)
  - slot y=768 → leave/logout (no tooltip read; not yet characterised)
2026-10-14T12:57Z  **Network finding (S-0003).** The titlebar persistently shows an "Offline" chip, but Browse loads live Modrinth data — real projects, download counts, follows, "3 days ago", 942 pages of results. The chip is `ref(!navigator.onLine)` (`apps/app-frontend/src/App.vue:465`), which is unreliable inside a Tauri webview; Tauri's Rust HTTP client is unaffected. So "Offline" is a **UI-level false positive**, and any control gated on it is wrongly disabled while the network is actually fine. Important for us: our own online indicator must not be a browser-API guess.
2026-10-14T12:58Z  i18n finding: the app ships **33 locales** under `apps/app-frontend/src/locales/` (ar-SA … zh-TW), Crowdin-managed.
2026-10-14T12:58Z  **Settings is a modal dialog with its own left nav, not a tab.** Sections observed: DISPLAY (Appearance, Features, Behavior, Language[Beta]), ACCOUNT (Profile, Social, Privacy), INSTANCES (Synced settings, Java installations, Resource management). Footer shows "Modrinth App 0.21.9 / Linux 24.4.0" — **in-app version matches the binary and the pinned tag**, closing the §10 version-drift question.
2026-10-14T12:59Z  Wrote `tools/mrcap_sweep.py` and started the Settings sweep: 10 sections × captured frame + scrolled frames, clicked by OCR'd label only.
2026-10-14T13:00Z  First sweep run produced **5 invalid frames**. It was launched while the app was on the Hosting tab (the rail walk's last click had navigated away), so every section click missed and `S-0010…S-0014` are duplicates of `S-0006`. **Ids retired, not reused; files deleted.** Fixed the tool to assert the dialog is open before sweeping, and to write scrolled frames into `screenshots/` as first-class ids.
2026-10-14T13:01Z  **The single most valuable source read of the mission.** The app's settings are not a JSON file: they are one row in a SQLite table. Read the live `app.db` `settings` schema and values straight off the running install — 45 columns, every default, plus two values where **live ≠ schema default** and both explained by migrations (`mc_memory_max` 4096 vs 2048; `version` 3 vs 1). `SOURCE packages/app-lib/src/state/settings.rs:10`.
2026-10-14T13:02Z  Read the live DB's table list: **59 tables**, 49 migrations, 601 cache rows on a fresh install. Ground truth for `data-model.md`.
2026-10-14T13:03Z  Re-ran the sweep with single-token needles. **Root cause of two earlier misses:** OCR splits "Synced settings" into two words, so a two-word needle never matches a token. Single-token needles fixed it — `Synced` → 9 frames, `Java` → 3, `Resource` → 9.
2026-10-14T13:05Z  Worker output landed: `modules/01` (66 KB), `03` (95 KB), `04` (93 KB), `05` (112 KB), `07` (38 KB), `08` (62 KB) — all `SOURCE`-cited, several with mid-draft self-corrections recorded. The orchestrator wrote `06` (20 KB), `data-model`, `api-surfaces`, `gap-analysis`, `roadmap`, `INDEX`, `MANIFEST`, `sources`, `open-questions`, and later `02`, `09`, `10`.
2026-10-14T13:06Z  **Note on method.** Two subagent slices returned without writing anything, and a later batch failed on rate limits. What worked: short, single-file briefs with an explicit "write the file and `ls -la` it before replying" instruction. Two workers' output was substantially better than the orchestrator's on the same topic (real locale strings, migration files, Tauri command names) — the worker version of `modules/06` is the one on disk.
2026-10-14T13:07Z  Wrote `gap-analysis.md` (top-ten gaps, per-surface mapping, and an explicit "what we already do better" section) and `roadmap.md` (11 slices with acceptance criteria, and the rule: **ship 0, 1, 3, 4 and 9 if only five ever ship**).
2026-10-14T13:08Z  Wrote `api-surfaces.md` from the typed client wrapper — `GET /search` with `facets: string[][]`, the five real sort indexes, `DependencyType` (4 values) and `Environment` (4 values), all `path:line`-cited, each with a mirror/skip call.
2026-10-14T13:09Z  Wrote `BLUEPRINT.md` (merged spec, top-to-bottom) and `FINAL-REPORT.md` (≤2 pages for the owner).
2026-10-14T13:10Z  Wrote `modules/02-home.md`, `09-updates-maintenance.md`, `10-cross-cutting.md` to close the last three gaps.
2026-10-14T13:11Z  **Deliverable complete.** 11 docs + 10 modules + 4 tools + 65 screenshots. Nothing committed — see `open-questions.md` OQ-011 for the commit decision the owner should make.