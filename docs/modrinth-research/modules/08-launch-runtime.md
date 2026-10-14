# 08 — Launch and runtime

This is the head of the deliverable the mission asked for: *how does this app
name problems?* Everything below is reconstructed from source at the pinned
`modrinth/code` commit `6ebd75b68dd386cd4d74eee1cf2e96d945c937db` (`v0.21.9`).

## Coverage table

| ID | label | type | evidence | status |
| --- | --- | --- | --- | --- |
| LAUNCH-1 | Play button state machine | button | `apps/app-frontend/src/pages/instance/components/page-header/index.vue:73-145` | SOURCE |
| LAUNCH-2 | Launch pipeline | state | `packages/app-lib/src/api/instance/run.rs:23-308`, `packages/app-lib/src/launcher/mod.rs:878-1314` | SOURCE |
| LAUNCH-3 | Pre-launch gates | state | `packages/app-lib/src/launcher/mod.rs:893-907`, `:1018-1053`, `:1207-1235` | SOURCE |
| LAUNCH-4 | Install job state machine | state | `packages/app-lib/src/install/model.rs:378-465`, `:497-503`, `:716-736` | SOURCE |
| LAUNCH-5 | Download manager (progress surface) | indicator | `apps/app-frontend/src/components/ui/download-manager/index.vue:1` | SOURCE |
| LAUNCH-6 | Legacy loading bars | indicator | `packages/app-lib/src/event/mod.rs:285-333` | SOURCE |
| LAUNCH-7 | Process lifecycle events | state | `packages/app-lib/src/event/mod.rs:409-412`, `packages/app-lib/src/state/process.rs:340-346`, `:945-951` | SOURCE |
| LAUNCH-8 | Concurrent launches | state | `packages/app-lib/src/launcher/mod.rs:1018-1033`, `:1229-1235`; `packages/app-lib/src/state/mod.rs:74` | SOURCE |
| LAUNCH-9 | Stop behaviour | button | `apps/app-frontend/src/pages/instance/layout.vue:687-700`; `packages/app-lib/src/state/process.rs:384-390` | SOURCE |
| LAUNCH-10 | Post-exit behaviour | state | `packages/app-lib/src/state/process.rs:906-1026` | SOURCE |
| LAUNCH-11 | Crash handling and diagnosis | dialog | `apps/app-frontend/src/pages/instance/logs/index.vue:111-126`, `:204-218` | SOURCE |
| LAUNCH-12 | Error taxonomy → user copy (headline) | — | `packages/app-lib/src/install/runner.rs:1805-1879`; `.../use-install-job-display.ts:213-241` | SOURCE |
| LAUNCH-13 | Java missing / broken / too old | dialog | `packages/app-lib/src/launcher/mod.rs:976-987`; `packages/app-lib/src/util/jre.rs:334-362` | SOURCE |
| LAUNCH-14 | Disk full | dialog | `packages/app-lib/src/state/content_store/adapters/filesystem.rs:243-254`; `apps/app-frontend/src/components/ui/ErrorModal.vue:73-84` | SOURCE |
| LAUNCH-15 | Corrupted download | indicator | `packages/app-lib/src/error.rs:119-120`; `.../use-install-job-display.ts:163-170` | SOURCE |
| LAUNCH-16 | No internet | indicator | `packages/app-lib/src/error.rs:104-108`; `.../use-install-job-display.ts:333-339` | SOURCE |
| LAUNCH-17 | No loader version (the one good repair flow) | dialog | `apps/app-frontend/src/components/ui/ErrorModal.vue:85-89`, `:275-283` | SOURCE |
| LAUNCH-18 | Missing file at launch → silent auto-repair | state | `packages/app-lib/src/launcher/mod.rs:1035-1053`; `.../download.rs:224-264` | SOURCE |
| LAUNCH-19 | Pre-launch / wrapper / post-exit hooks | — | `packages/app-lib/src/launcher/hooks.rs:1-93`; `packages/app-lib/src/api/instance/run.rs:106-236` | SOURCE |
| LAUNCH-20 | Arguments and environment injected into the game | — | `packages/app-lib/src/launcher/args.rs:111-212`, `:330-360`; `.../launcher/mod.rs:1133-1176` | SOURCE |
| LAUNCH-21 | Live observation of any launch state | — | needs a real Minecraft account and a game that can run | **[ORCH-PENDING]** — never performed |

---

## The state machines, confirmed from source

### LAUNCH-1 — Play button state machine
**Type:** button
**Nav path:** `/instance/:id` — the page header's action slot, right of the instance icon.
**Evidence:** [SOURCE `apps/app-frontend/src/pages/instance/components/page-header/index.vue:73-145`, `:339-345`]
**Purpose:** The single control that means "make this game run", which is a five-way branch rather than one button.
**Operation:** exactly six mutually exclusive renderings, in priority order:

| Order | Condition | What the user sees | Cancellable |
| --- | --- | --- | --- |
| 1 | a process is running for this instance | red "Stop" button, disabled and reading "Stopping..." while the kill is in flight | yes |
| 2 | `install_stage` is `installing` \| `pack_installing` \| `minecraft_installing` | disabled "Installing..." | no |
| 3 | instance is quarantined | disabled "Play", tooltip "This instance has been locked" | n/a |
| 4 | `install_stage !== 'installed'` | **"Repair"** with a download glyph — not "Play" | no |
| 5 | server-linked instance and not loading | split button "Play" with a "Launch instance" dropdown | n/a |
| 6 | otherwise and not loading | "Play" | n/a |
| — | otherwise (loading) | disabled "Starting..." | no |

Note that there is **no separate "verifying" or "downloading" state on the button itself** — the typical `idle → verifying → downloading` sequence the brief anticipated does not exist here. Downloading is a *global* job surfaced elsewhere (LAUNCH-5), and the button only reflects it indirectly through branch 2. That is a real finding: the button has one coarse "busy" signal, not a pipeline readout.
**Controls:** Play; Stop; Repair; the server-launch dropdown; a settings icon and an overflow menu (Open folder, Export modpack, Create shortcut, Report — the last only for shared-instance members).
**Persistence:** n/a.
**Rationale & notes:** Branch 4 is the good decision — a not-installed instance offers "Repair", an imperative that names the actual state, instead of a "Play" button that would fail. Branch 3 turns the control into a labelled dead end with an explanation rather than hiding it. Branch 6's "Starting..." is set purely by a frontend `loading` ref around the launch RPC; it is not driven by a backend phase, so a slow launch shows only that one word. Also worth noting: `isInstalling` tests the *instance's* stage, but the actual download runs as a job (LAUNCH-4), so the button can show "Installing..." from a stale `install_stage` while nothing is downloading.

---

## LAUNCH-2 — Launch pipeline
**Type:** state
**Nav path:** Play on `/instance/:id`, the instance context menu, "Play server" from a Modrinth server project, or the repair flow.
**Evidence:** [SOURCE `apps/app-frontend/src/pages/instance/layout.vue:578-596`, `:637-685`; `apps/app/src/api/instance.rs:1322-1332`; `packages/app-lib/src/api/instance/run.rs:23-308`; `packages/app-lib/src/launcher/mod.rs:878-1314`]
**Purpose:** Turn "play this instance" into a spawned `java` process, handling everything that has to happen first.
**Operation:** the real sequence, from the frontend inward.

1. **`startInstance(context)`** (frontend) — re-entrancy guards on `checkingSharedInstanceLaunch`, `loading`, `playing`; then two optional *pre-launch interstitials* that are themselves gates, not pipeline steps: a shared-instance update check (if the instance is a shared instance and the app is online), then the "update to play" modal when content updates are pending. Either can cancel the launch or defer it behind its own modal.
2. **`launchInstance(context)`** — sets `loading = true`, calls `run(instanceId)`, optimistically writes `[true]` into the processes query, clears `loading`, fires an analytics event.
3. **`instance_run` → `run(instance_id, quick_play_type)`** — rejects a quarantined instance ("This instance has been quarantined"); takes a legacy-content-migration write lock; checks shared-instance availability; then **requires Minecraft credentials**, failing with `NoCredentialsError` if absent.
4. **`run_credentials`** — resolves settings and the launch context, applies per-instance overrides over global settings (memory, resolution, JVM args, env vars, all three hooks), then runs the pre-launch hook and waits for it (LAUNCH-19).
5. **Session join (servers only)** — if the instance is linked to a Modrinth server project, POSTs to Mojang's session server with a random `serverId`, then reports the play to Modrinth analytics. Failures are `warn!`-only.
6. **Skin flush** — `flush_pending_skin_change()` so a UI skin selection is not lost (see AUTH-12).
7. **`launch_minecraft`** — LAUNCH-3's gates, then argument assembly (LAUNCH-20) and process insertion.
8. **`insert_new_process`** — creates/truncates `logs/launcher_log.txt` with a header, spawns the child with stdin/stdout/stderr piped, persists a `processes` row, spawns stdout and stderr readers, registers the process, starts the sequential process manager, and emits `ProcessPayloadType::Launched` ("Launched Minecraft").
9. **RPC handshake** — a `theseus.jar` java agent is attached; before `launch()` is called the launcher pushes system properties into the game over a local socket: process start time, instance created/modified timestamps, instance name, icon path, and the linked project/version ids.

**Controls:** the QuickPlay type (`None` / `Singleplayer(world)` / `Server(address)`), which becomes either injected `-Dmodrinth.internal.quickPlay.*` properties or legacy `--server`/`--port` arguments depending on the game version.
**Persistence:** `processes` row (pid, start time, name, executable, instance id, post-exit command); `last_played` timestamp.
**Rationale & notes:** The two interstitials before `run()` are the interesting part — the app treats "content has changed" and "content needs updating" as *negotiable preconditions* rather than as part of launching, which is what keeps the button's state machine as small as LAUNCH-1 shows. The persistence of the process row is also what lets the app refuse a second launch after a crash-restart cycle: `instance_has_running_process` cross-checks the row against `sysinfo` with a 2-second start-time tolerance and deletes rows whose process is gone.

---

## LAUNCH-3 — Pre-launch gates
**Type:** state
**Nav path:** inside `launch_minecraft`, before spawn.
**Evidence:** [SOURCE `packages/app-lib/src/launcher/mod.rs:893-907`, `:1018-1053`, `:1207-1235`; `packages/app-lib/src/state/process.rs:102-149`]
**Purpose:** Refuse to launch in any state that would produce a broken or duplicated game, and self-heal the one recoverable case.
**Operation:** seven checks, in order, each a distinct `ErrorKind`:

1. `install_stage` is `pack_installing`/`minecraft_installing` → "Instance is still installing".
2. `install_stage` is not `installed` → "Instance is not installed; start an install job first".
3. no loader version resolved for a non-vanilla loader → "No loader version selected for {loader}".
4. no Java matching the version's major → "Missing correct java installation".
5. a process already exists in `process_manager` for this instance → "Instance {id} is already running as process {uuid}"; or one recorded in the `processes` table and still alive per `sysinfo` → "Instance {id} is already running".
6. **missing runtime file** → silently kicks off a full repair install job and *blocks the launch on it* (`install_existing_instance` + `wait_for_job`), then re-acquires the runtime lock and continues. Logged at `info`, not shown.
7. after re-syncing files, taking the instance-content lock and recovering the content store, it re-checks `install_stage` ("Instance is not ready to launch; finish or recover its installation first"), refuses a pending recovery, and checks for a running process a **second** time — the same check as 5, deliberately repeated after the long lock-taking section, because an instance could have been launched by someone else during it.

**Controls:** n/a.
**Persistence:** n/a.
**Rationale & notes:** Check 6 is the standout: a missing runtime jar or library at launch is not an error the user ever sees, it is an invisible repair-and-continue. That is right for the common case (a user cleaned their meta directory) and wrong for the rare one (a disk that is silently losing files), where the user instead experiences a mysteriously long pause before the game starts. Check 7's repetition is cheap insurance against a real race and is the kind of thing worth copying.

---

## LAUNCH-4 — Install job state machine
**Type:** state
**Nav path:** Download manager popover (top-right of the titlebar) — not the Play button.
**Evidence:** [SOURCE `packages/app-lib/src/install/model.rs:378-385`, `:445-465`, `:491-503`, `:711-736`; `packages/app-lib/src/install/control.rs:13-101`; `packages/app-lib/src/install/runner.rs:763-801`]
**Purpose:** Every download, repair, import and bulk update is a *job* with durable state, rather than a fire-and-forget task.
**Operation:** six **statuses** — `Queued`, `Running`, `Succeeded`, `Failed`, `Interrupted`, `Canceled` — and thirteen **phases** inside `Running`:

`preparing_instance` → `resolving_pack` → `downloading_pack_file` → `reading_pack_manifest` → `downloading_content` → `extracting_overrides` → `resolving_minecraft` → `resolving_loader` → `preparing_java` → `downloading_minecraft` → `running_loader_processors` → `finalizing` → `rolling_back`

`preparing_java` carries five sub-steps of its own: `Resolving`, `FetchingMetadata`, `Downloading`, `Extracting`, `Validating`. Each phase has a progress counter (`current`/`total`) plus an optional **secondary** counter — which is how file-count progress and byte progress coexist during `downloading_content`. Nine job kinds are distinguished (`create_instance`, `create_modpack_instance`, `create_shared_instance`, `import_instance`, `duplicate_instance`, `install_existing_instance`, `install_pack_to_existing_instance`, `update_shared_instance`, `bulk_update_content`), each with its own progress weighting table in the frontend.
`Interrupted` is distinct from `Failed` on purpose: it means the app closed mid-job, and its only offered action is Retry.
**Controls:** Pause / Resume / Cancel / Retry / Dismiss ("Remove from history") / Copy details.
**Persistence:** the full `InstallJobState` — request, target, cleanup plan, progress, paths, an **event log** (`JobQueued`, `JobStarted`, `PhaseStarted`, `ContentDownloadStarted`, `ContentFileSkipped{path,reason}`, `ContentFileCompleted{path,bytes,reused}`, `JobCanceled`, `Interrupted{reason,phase}`, `Failed{phase,code,message}`, `RollbackStarted/Completed/Failed`), an `InstallErrorView`, and rollback state.
**Rationale & notes:** Three decisions here are worth more than the rest of the module. First, `ContentFileSkipped` and `ContentFileCompleted` carry the **path**, so "this file already existed" and "this file downloaded" are separate recorded facts — the job log is a real audit trail, not a spinner. Second, `Interrupted` is a first-class status: a crash mid-install is not a failure, it is a resumable state with a Retry that restores the pre-install stage. Third, the phase taxonomy is *the reason LAUNCH-12 works at all* — because every phase is named, an error can be described as "what went wrong, during which step", which is exactly what the user copy does.

---

## LAUNCH-5 — Download manager (the launch-time progress surface)
**Type:** indicator
**Nav path:** a download glyph in the titlebar's top-right, opening a popover panel.
**Evidence:** [SOURCE `apps/app-frontend/src/components/ui/download-manager/index.vue:1`; `.../use-download-manager.ts:89-488`; `.../use-install-job-display.ts:243-443`; `.../install-job-progress.ts:1-97`; `packages/app-lib/src/install/events.rs:11-12`, `:158-198`]
**Purpose:** This is where "downloading" actually lives. It is a persistent, global, resumable task list rather than a per-launch progress bar — because a launch may trigger no job at all (LAUNCH-18) and an install may outlive the page you started it from.
**Operation:** three sections — **Active**, **Needs attention**, **Complete** — each row showing a 36 px instance avatar, the instance name as title, a plain-English status line, a progress bar, a progress label, an ETA, and per-row actions. The collapsed bar shows the selected job's title, an aggregate rate across all active jobs, an active-count badge, and colour-shifts to orange when something needs attention and to green on completion. A 1 Hz `setInterval` drives the ETA and rate readouts, and only runs while there is at least one active job. Transport is a Tauri `Channel` fed by an `install_job` event carrying a whole `InstallJobSnapshot`; the frontend reconciles event-vs-poll by comparing a monotonically increasing `modified` timestamp, and re-polls on `install_job_list(true)` at mount. Backend-side the reporter persists at most every **750 ms**, and during content downloads only every **25 files**, to keep SQLite writes down. A separate pseudo-job, "Content storage", shows a `Verifying` / `Verification complete` / `Some files still need attention` row driven by a `Channel` of `(current, total)` bytes.
**Phase → user-facing line** (`getText`), verbatim from the locale bundle: "Queued to install", "Resolving content", "Downloading pack file", "Reading pack manifest", "Downloading content", "Extracting overrides", "Resolving Minecraft", "Resolving loader", "Preparing Java", "Downloading Minecraft", "Running loader processors", "Finalizing", "Rolling back"; and for `preparing_java`, the sub-step line: "Preparing Java {version}", "Fetching Java {version}", "Downloading Java {version}", "Extracting Java {version}", "Validating Java {version}".
**Controls:** Pause installation / Resume installation / Cancel installation / Retry / Dismiss / Remove from history / Copy details / Open instance / Clear all completed tasks.
**Persistence:** the job records themselves survive restarts, so an interrupted job is still listed after relaunch.
**Rationale & notes:** Naming every phase in user language is what makes a long install legible — a user watching "Downloading pack file" knows something is happening and roughly what. The weighted overall-progress table (`preparing_java` is 12% of an instance install but 5% of a modpack install) is a deliberate lie that keeps the bar moving smoothly across heterogeneous phases. The 750 ms/25-file persist throttle is the sort of detail that separates a design that works from one that melts a disk. The main weakness: the whole surface is a *download* manager, so a user who presses Play on an instance whose content is being repaired sees nothing at the button and has to know to look at the titlebar.

---

## LAUNCH-6 — Legacy loading bars
**Type:** indicator
**Nav path:** a second titlebar popover beside the download manager, plus the splash screen.
**Evidence:** [SOURCE `packages/app-lib/src/event/mod.rs:285-333`; `packages/app-lib/src/event/emit.rs:45-155`; `apps/app-frontend/src/components/ui/AppActionBar.vue:373-499`]
**Purpose:** A parallel, simpler progress mechanism predating the job system, still live for operations that are not installs.
**Operation:** an `init_loading(type, total, title)` call registers a bar and immediately emits 0%; subsequent `emit_loading(&bar, fraction, message)` calls advance it. Thirteen bar types exist — `LegacyDataMigration`, `DirectoryMove{old,new}`, `JavaDownload{version}`, `PackFileDownload`, `PackDownload`, `PackImport`, `MinecraftDownload{instance_id,instance_name}`, `ZipExtract`, `PackExport`, `ConfigChange`, `CopyInstance`, `LauncherUpdate{version,current_version}` — and the titlebar popover renders each with its own title ("Downloading Java {version}" for `java_download`, the instance name or pack name for the rest) and a progress bar. The download-manager bar **filters these out**: `java_download`, `pack_file_download`, `pack_download`, `minecraft_download`, `copy_instance` and `launcher_update` are all excluded from the legacy list (`AppActionBar.vue:452-464`) precisely because the job system now owns them.
**Controls:** none — these bars are not cancellable.
**Persistence:** none.
**Rationale & notes:** Two systems coexisting is a transitional cost, and the exclusion list is the seam: the new system claims each legacy bar type in turn. "Safe" bars also register with the app's exit path, so the app waits for them before quitting — which is why the `init_loading` doc comment says so explicitly. Worth copying the *idea* (a distinct, non-cancellable progress channel for one-shot maintenance operations) but not the duplication.

---

## LAUNCH-7 — Process lifecycle events
**Type:** state
**Nav path:** n/a — background.
**Evidence:** [SOURCE `packages/app-lib/src/event/mod.rs:408-412`; `packages/app-lib/src/state/process.rs:340-346`, `:945-951`; `apps/app-frontend/src/pages/instance/layout.vue:910-919`; `.../AppActionBar.vue:296-298`]
**Purpose:** Two events carry the entire runtime state to the UI. There is no "crashed" event.
**Operation:** `ProcessPayloadType` has exactly two variants. `Launched` ("Launched Minecraft") is emitted once, synchronously, *after* the child is spawned and its readers are running. `Finished` ("Exited process") is emitted by the sequential process manager once the child's exit status is observed. Both carry `instance_id` and the process `uuid`. The frontend reacts by flipping the processes query cache to `[true]` or `[]`, invalidating the console and log queries, clearing the live console buffer on launch, and re-reading the account card on launch.
**Controls:** n/a.
**Persistence:** n/a.
**Rationale & notes:** Two events for what is conceptually a three-state machine is the single most consequential simplification in this module. `Finished` does **not** carry the exit status and does **not** distinguish a crash from a normal quit — the UI cannot know. Everything crash-shaped happens later, in the log analysis (LAUNCH-11). For a launch pipeline this is a defensible simplification (exit status is rarely actionable to a user) but it is exactly the seam where a better launcher could do more.

---

## LAUNCH-8 — Concurrent launches
**Type:** state
**Nav path:** n/a.
**Evidence:** [SOURCE `packages/app-lib/src/launcher/mod.rs:1018-1033`, `:1229-1235`; `packages/app-lib/src/state/mod.rs:74`, `:384`; `packages/app-lib/src/install/runner.rs:774-781`; `apps/app-frontend/src/components/ui/AppActionBar.vue:23-120`]
**Purpose:** Bound resource use without serialising the whole app.
**Operation:** two separate limits, and they are not the same thing.

- **Game processes:** there is **no global limit and no queue.** N different instances may run simultaneously. The only refusal is per-instance: a second launch of an instance that already has a live process is rejected (twice — see LAUNCH-3). `ProcessManager` is a `DashMap<Uuid, Process>` keyed by process uuid, so it holds any number.
- **Install jobs:** there **is** a queue and a hard cap of **3 concurrent jobs** (`MAX_CONCURRENT_INSTALL_JOBS: usize = 3`). A fourth job sits in `Queued` holding a semaphore permit it has not acquired; the permit is raced against cancellation, so cancelling a queued job does not wait for a slot. A per-target mutex additionally serialises jobs that touch the same instance.

The UI reflects the first of these: the titlebar shows the selected running instance by name, and when more than one is running a chevron opens a list of all of them, each with a stop button and a "view logs" link, and each markable as the "Primary instance". With nothing running it reads "No instances running".
**Controls:** per-process Stop; per-process View logs; make-primary.
**Persistence:** none.
**Rationale & notes:** The two-tier design is the interesting part. Downloads are bandwidth- and disk-bound, so a cap of three is right; the game is the user's choice, so no cap. The queue is a real queue with a visible `Queued` status and a "Queued to install" label, not a hidden one. The primary-instance selector is a small nicety that most launchers omit — with several games running, the titlebar needs *one* of them to represent "the one you care about", and letting the user choose beats picking by recency.

---

## LAUNCH-9 — Stop behaviour
**Type:** button
**Nav path:** the red "Stop" button on the instance page header; the per-row stop control in the titlebar popover; the "Stop" item in the instance context menu.
**Evidence:** [SOURCE `apps/app-frontend/src/pages/instance/layout.vue:687-700`; `apps/app-frontend/src/components/ui/AppActionBar.vue:300-313`, `:101-107`; `packages/app-lib/src/api/instance/run.rs:325-335`; `packages/app-lib/src/api/process.rs:66-73`; `packages/app-lib/src/state/process.rs:384-390`]
**Purpose:** Stop a game the user no longer wants, without leaving a zombie process.
**Operation:** the instance page calls `kill(instanceId)`, which resolves every process for that instance and calls `ProcessManager::kill(uuid)` on each. That calls `tokio::process::Child::kill()` and awaits it — which is `start_kill()` then `wait()`, so it blocks until the child is reaped. The titlebar instead kills a single process by uuid. The instance page sets `stopping = true` (the button becomes a disabled "Stopping...") for the duration, then clears the processes query cache.
**Controls:** Stop (no confirmation).
**Persistence:** none — the `processes` row is cleared by the sequential process manager when it observes the exit, not by the kill.
**Rationale & notes:** `tokio`'s `Child::kill()` is a hard kill — `SIGKILL` on Unix, `TerminateProcess` on Windows (`Cargo.lock:12743-12745` pins tokio 1.53.0; the signal semantics are tokio's documented behaviour, **INFERRED**, since only `child.kill().await` is called here) — and because `kill()` is `start_kill()` followed by `wait()`, it does block until the child is reaped. There is **no graceful path**: no window-close message, no `stdin` command, no signal escalation, no save-and-wait. For a launcher that has its own `main class` inside the game process and an RPC channel to it (LAUNCH-2 step 9), a graceful "save and quit" would be possible and is conspicuously absent.

The user-visible consequence is that stopping mid-save can lose world progress, and the app does not warn about that and does not ask for confirmation. The sequencing is otherwise correct: the instance page awaits `kill()` before clearing its processes cache, and the sequential process manager observes the same exit independently on its 50 ms poll, so playtime finalisation, log teardown and the post-exit hook all still run. What the user does not get is any acknowledgement beyond the button reverting to "Play" — and with a 50 ms poll floor between the kill and the `Finished` event, the Stop button's "Stopping..." label exists for a very short window.

Two smaller observations. The stop control is context-dependent by *surface* rather than by capability: the instance page kills **every** process for the instance (a loop), while the titlebar kills **one** process by uuid — the right behaviour when only one game is running, subtly different when it is not. And there is no confirmation anywhere, on a control whose consequence (lost unsaved progress) the user cannot predict.

---

## LAUNCH-10 — Post-exit behaviour
**Type:** state
**Nav path:** n/a — background.
**Evidence:** [SOURCE `packages/app-lib/src/state/process.rs:860-1026`]
**Purpose:** Close out everything the running game was holding, and leave the app in a clean, re-launchable state.
**Operation:** a background task per process polls `try_wait` every **50 ms**; when it yields a status the sequence is:

1. remove the process from `process_manager` and delete its `processes` row;
2. spawn a task to reconcile synced instance options (upload/dirty-tracking);
3. emit `Finished`;
4. finalise playtime and spawn an upload of the playtime delta to Modrinth analytics;
5. append `\n# Process exited with status: {status}\n` to `logs/launcher_log.txt`;
6. clear Discord rich presence and friends presence;
7. if the *refocus* setting is on, un-minimise and focus the main window;
8. **only if `exit_status.success()`**, spawn the post-exit hook (or hook command) with the injected env, `current_dir` set to the instance directory, and **do not wait for it** — it is fire-and-forget.

**Controls:** the "Refocus app when Minecraft closes" toggle (default off; description "Bring Modrinth App to the foreground when Minecraft exits"), and "Minimize app" / "Minimize Modrinth App when Minecraft starts." (`hide_on_process_start`).
**Persistence:** exit status appended to `logs/launcher_log.txt`; playtime accumulated in the instance row.
**Rationale & notes:** Three good calls. The exit status *is* recorded — in the launcher log, right where a user would look — even though it is never surfaced in the UI (LAUNCH-07). Playtime is flushed once more at exit so a sub-minute session is not lost, and the analytics upload is spawned separately so a slow network call cannot delay the app noticing the game closed. And the post-exit hook is gated on `success()`, so a crash does not trigger arbitrary user-configured commands. The behaviour the brief anticipated — "offer to open logs, refresh state, prompt to relaunch" — does **not** exist. There is no post-exit dialog of any kind. State refresh is silent and automatic via the `Finished` event.

---

## LAUNCH-11 — Crash handling and diagnosis
**Type:** dialog (conditional)
**Nav path:** `/instance/:id/logs` — a crash-insight card appears in the console when the game is not running.
**Evidence:** [SOURCE `apps/app-frontend/src/pages/instance/logs/index.vue:111-126`, `:146-165`, `:190-192`, `:204-218`; `packages/api-client/src/modules/mclogs/insights/v1.ts:1-19`; `packages/api-client/src/modules/mclogs/types.ts`; `packages/app-lib/src/state/dirs.rs:186-191`; `packages/app-lib/src/api/logs.rs:26-30`, `:269-342`]
**Purpose:** Name the crash in the user's terms, automatically, without the user having to know what a stack trace is.
**Operation:** this is the closest thing to a crash report the app has, and it is worth being precise about what it is and is not.

**There is no crash dialog, no crash-report file generation, no upload, and no exit-code analysis.** A "crash report" here is simply a file the *game* wrote into `<instance>/crash-reports/`, which the log browser lists alongside `<instance>/logs` as a second `LogType::CrashReport` source; those files are read, run-length-compacted, and censored (OS username → `{COMPUTER_USERNAME}`, access token → `{MINECRAFT_ACCESS_TOKEN}`, username → `{MINECRAFT_USERNAME}`, UUID → `{MINECRAFT_UUID}`) before display. Repeated identical lines (20+) are collapsed to `line (xN times - compacted by Modrinth App)` and, if any collapsing happened, a warning notification says how many lines were compacted in which file — an honest disclosure that the displayed log is not the raw log.

The actual diagnosis: when the Logs tab mounts while the instance is *not* running, or when a `Finished` event arrives, the live log is posted to `https://api.mclo.gs/1/analyse` (`skipAuth`, form-encoded). The response's `analysis.problems[]` is rendered by a `crashAnalysis` card passed into the shared console layout, with a dismiss control. Failures are swallowed — the comment says "Crash analysis is best-effort". The returned shape gives per-problem: a `message`, a `counter` (occurrences), the offending `entry` (log level, time, prefix, and the exact numbered lines), and a `solutions[]` list. A separate `information[]` array carries non-fatal context.
**Controls:** dismiss the analysis card; switch between "Live Log" and each historical file; delete a historical file (`latest.log` is undeletable while the instance is running).
**Persistence:** none — analysis is not stored, and the live log lives only in a 50,000-line in-memory ring buffer per instance.
**Rationale & notes:** This is the single best idea in the whole runtime slice, and it is worth copying in our own words. The insight is that a launcher's most valuable crash asset is not the exit code — which nobody can act on — but the **text of the log**, and that a pattern-matching service can turn that text into "here is what happened, here is the exact line, here is what to try". Tying it to the `Finished` event means it appears at the exact moment the user goes looking. The compromises: it depends on a third-party service (mclo.gs), so it fails closed; it only runs when the Logs tab is actually open or mounted, so a user who never visits Logs never sees it; and the analysis is ephemeral, so it cannot be attached to a later support request. Note also the deliberate privacy work — logs are censored before they leave the process and again before they are displayed.

---

## LAUNCH-12 — Error taxonomy → user copy *(headline deliverable)*
**Type:** —
**Nav path:** the Download manager's "Needs attention" rows; the generic error modal; the auth modals.
**Evidence:** [SOURCE `packages/app-lib/src/install/runner.rs:1788-1879`; `apps/app-frontend/src/components/ui/download-manager/use-install-job-display.ts:130-361`; `packages/app-lib/src/error.rs:47-227`]
**Purpose:** Answer the question "how does this app name problems?" — and the answer is: with a two-stage vocabulary that almost no launcher bothers with.
**Operation:** the machine emits a **code**, and the frontend translates it to a **sentence**. The code is chosen from the `ErrorKind` *and* the phase it happened in (`install_error_code`, `runner.rs:1805-1879`), producing one of: `api_error`, `archive_error`, `parse_error`, `content_error`, `hash_error`, `filesystem_error`, `path_error`, `instance_error`, `java_error`, `loader_error`, `processor_error`, `database_error`, `network_error`, `pack_error`, `metadata_error`, `launcher_error`, `shared_instance_unavailable`, `shared_instances_api_error`, `internal_error`, `rollback_error`, plus the job-level `canceled`, `app_closed` and `interrupted`.

The frontend then resolves a sentence in this priority order:

1. `app_closed` or `interrupted` → **"Canceled due to app closing"**
2. a rollback error → **"Cleanup didn't finish"**
3. `canceled` → **"Canceled"**
4. the message contains `permission denied` / `access is denied` / `operation not permitted` → **"No permission to write"** (a substring sniff that overrides the code — the one place the app pattern-matches a raw OS message, and it is a good one)
5. `network_error` → **"Download couldn't finish"**, or **"Couldn't download pack"** if the phase was `downloading_pack_file`
6. `pack_error` → **"Modpack data invalid"**, or **"Couldn't download pack"** in that phase
7. otherwise, code first, then phase as a fallback, then **"Something went wrong"**.

Code → sentence: `api_error` → "Couldn't reach Modrinth" · `archive_error` → "Couldn't read modpack" · `parse_error` → "Modpack data invalid" · `content_error` → "Modpack files have invalid metadata" · `hash_error` → "Downloaded file is corrupt" · `filesystem_error` → "Couldn't save files" · `path_error` → "File path is invalid" · `instance_error` → "Couldn't prepare instance" · `java_error` → "Java setup couldn't finish" · `loader_error` and `processor_error` → "Loader setup failed" · `database_error` → "Couldn't update local data".

Phase → sentence (only when the code is unmapped): `downloading_pack_file` → "Couldn't download pack" · `resolving_pack`/`reading_pack_manifest` → "Modpack data invalid" · `downloading_content` → "Couldn't download files" · `extracting_overrides` → "Couldn't save files" · `resolving_minecraft`/`downloading_minecraft` → "Minecraft setup failed" · `resolving_loader`/`running_loader_processors` → "Loader setup failed" · `preparing_java` → "Java setup couldn't finish" · `preparing_instance` → "Couldn't prepare instance" · `rolling_back` → "Cleanup didn't finish".
**Controls:** Retry; Copy details; Dismiss.
**Persistence:** the code, phase, message, a structured `InstallErrorContext` (operation, file path, target path, entry path, URLs, expected hash and size, project/version ids, Minecraft version, loader, Java version, OS, arch) and the raw API error (status, method, url, route) are all persisted on the job record.
**Rationale & notes — this is the deliverable.** Four things make this genuinely good, and we should match its helpfulness in our own words:

1. **A bounded, non-technical sentence list.** Twenty user-facing strings, all short, all in the user's mental model, none of them mentioning a Rust enum. Compare a typical launcher, which surfaces `ErrorKind::HashError(expected, actual)`.
2. **The phase disambiguates.** The same error code yields a different sentence depending on where it happened. That is what lets a flat list of twenty strings cover an enormous space, and it is the mechanism a smaller project can most cheaply copy.
3. **Permission errors are sniffed, not classified.** The backend cannot reliably classify an OS error, so the frontend looks for three substrings and reports "No permission to write" — an outcome the user can act on — rather than "Couldn't save files".
4. **Every failure carries its context.** The persisted context names the file, the URL, the expected hash and size. This is what makes "Copy details" produce something a support agent can act on rather than a wall of text.

**Copy details** is the piece that ties it together. It is not the raw error — it is a generated support document containing: a title, a one-line result ("interrupted because the app closed while downloading content", "failed while downloading content (network_error)"), the job id, the request kind, the status, the current phase and progress, an environment block (app version, OS name/version/kind/family, architecture, kernel), a numbered **timeline** reconstructed from the event log, a content summary, the errors, the raw JSON snapshot, and a tail of the launcher log. It is then run through a censoring pass before it leaves the process. A user who has hit a bug the app cannot explain is handed a document that is already 80% of a support ticket.

**Where the naming is weak, honestly.** A disk that is full is called "Couldn't save files" — which is true, unhelpful, and the one case where a more specific sentence was available and not taken (see LAUNCH-14). A launcher-stage failure outside the install system has no code at all and falls through to the generic error modal's "An error occurred". And the phase taxonomy, good as it is, has no phase for "the game started and then died", which is precisely the state the brief asked about.

---

## LAUNCH-13 — Java missing, broken, or too old
**Type:** dialog / indicator
**Nav path:** auto-raised on failure during install or launch; manually reachable via Settings → Instances → "Java installations", and the per-instance Java tab.
**Evidence:** [SOURCE `packages/app-lib/src/launcher/mod.rs:504-563`, `:976-987`; `packages/app-lib/src/util/jre.rs:253-332`, `:334-362`; `packages/app-lib/src/api/jre.rs:63-209`; `apps/app-frontend/src/components/ui/JavaDetectionModal.vue:1`]
**Purpose:** A launcher is only as good as its ability to find or fetch a working JRE, because every Minecraft version demands a specific major version.
**Operation:** the *normal* path is fully automatic and produces no error at all. The required major version comes from the version manifest (`javaVersion.majorVersion`, defaulting to 8 when absent). If a matching JRE is already registered it is used; otherwise the app queries Azul's metadata API for a matching Zulu JRE, downloads the zip, extracts it into `meta/java_versions/`, registers it, and reports each sub-step (see LAUNCH-4's five Java steps). If the instance has a user-set Java path, that is checked first and falls back to auto-install on failure.

Only when this all fails does the user see anything, and then it is one of six `JREError` variants: "No executable found at {path}", "Could not check Java version at path {path}", "Invalid JRE version string: {s}", "Parsing error: {0}", "Env error: {0}", "Join error: {0}", "Command error: {0}", plus "No stored tag for Minecraft version {0}" and "Error getting launcher state". In the install system these become code `java_error` → **"Java setup couldn't finish"**. In the launcher path they become "Missing correct java installation" or a `JRE error: …` message routed to the generic modal.

The recovery surface is genuinely good and separate from the error: a "Select Java installation" modal lists every detected JVM with its version and path and a per-row Select button, with the empty state "No Java installations found." Detection covers `PATH`, `JAVA_HOME`, platform-specific well-known locations, and on Windows the registry (both 32- and 64-bit views). Each candidate is probed by *running it* — `java -cp theseus.jar com.modrinth.theseus.JavaInfo` — and parsing `java.version` and `os.arch` out of stdout, which is the only reliable probe. A separate control offers "Install recommended", "Detect", "Browse" and "Test Java installation".
**Controls:** auto-install recommended; Detect; Browse for a path; Test Java installation; per-row Select.
**Persistence:** registered JREs in the `java_versions` table (major version, full version, architecture, path).
**Rationale & notes:** The headline is that **Java problems are designed not to happen**: the app downloads and validates the exact JRE the version needs, so a user with no Java at all is never blocked. Probe-by-execution rather than probe-by-directory-shape is correct and almost universally skipped. Where it is weak: there is no specific user-facing sentence for "Java too old" — a JRE that probes successfully but cannot run the game surfaces as a generic launcher error or as a game crash with no explanation. And the "Test Java installation" control only checks that a version string can be read, not that the JRE can actually load the game.

---

## LAUNCH-14 — Disk full
**Type:** indicator (naming); dialog (only for the directory-move case)
**Nav path:** any install; or Settings when changing the app directory.
**Evidence:** [SOURCE `packages/app-lib/src/state/content_store/adapters/filesystem.rs:243-254`; `packages/app-lib/src/state/dirs/move_app_directory.rs:479-488`; `packages/app-lib/src/error.rs:55-56`, `:95-96`; `apps/app-frontend/src/components/ui/ErrorModal.vue:73-84`, `:241-247`]
**Purpose:** Fail before filling a disk rather than after.
**Operation:** the content store pre-checks free space before staging any file, requiring the file's size plus a 64 MB working margin, and raises `io::ErrorKind::StorageFull` with the message "There is not enough space to import this file into the shared content store". The directory-move code has its own check with the message "Not enough space: the destination needs at least {n} bytes plus working space for the app-directory move".
Both surface badly in the install path: `StorageFull` is an `io::Error`, which becomes `StdIOError`, which becomes code `filesystem_error`, which becomes **"Couldn't save files"**.
The *only* place the app names it properly is the directory-move flow, where `ErrorModal` matches the substring "Not enough space" and shows a dedicated panel: **"Not enough space — It looks like there is not enough space on the disk containing the directory you selected. Please free up some space and try again or cancel the directory change."** with "Retry directory change" and "Cancel directory change".
**Controls:** (install) Retry / Copy details; (directory move) Retry / Cancel.
**Persistence:** n/a.
**Rationale & notes:** This is the clearest naming defect in the whole slice, and it is instructive precisely because the app *has* the information and throws it away on the common path. A pre-flight space check runs, its specific message is produced, and then it is flattened into "Couldn't save files" — a sentence that describes a symptom, not a cause, and gives the user nothing to act on. The directory-move panel proves the team knows better. **Our launcher should name the cause, not the symptom, in every case where a pre-flight check already knows the answer.** The pre-flight itself is excellent and should be copied: check before you write, with a working margin, rather than failing half-way through a download.

---

## LAUNCH-15 — Corrupted download
**Type:** indicator
**Nav path:** Download manager, "Needs attention".
**Evidence:** [SOURCE `packages/app-lib/src/error.rs:119-120`; `packages/app-lib/src/install/runner.rs:1861`; `apps/app-frontend/src/components/ui/download-manager/use-install-job-display.ts:163-170`; `packages/app-lib/src/state/content_store/adapters/filesystem.rs:266-268`]
**Purpose:** Content-addressed storage makes corruption detectable, so corruption should never reach the game.
**Operation:** the store computes `sha512` and `size` for every file it stages. A mismatch raises `ErrorKind::HashError(expected, actual)`, whose Display is "Incorrect Sha1 hash for download: {expected} != {actual}" (the variant name says SHA-1, the implementation says SHA-512 — a small, telling inconsistency). That maps to code `hash_error` and the sentence **"Downloaded file is corrupt"**, offered with **Retry**. Independently, the manual "Content storage" verification pass (LAUNCH-5) walks the whole store comparing stored hashes and can **repair** what it finds, reporting "Verification complete" or "Some files still need attention" with the offending sha512 and message.
**Controls:** Retry; the settings-level Verify-and-repair pass.
**Persistence:** sha512 + size per content entry.
**Rationale & notes:** Naming this correctly is what makes it useful: "Downloaded file is corrupt" tells the user the problem is transient and in *their* download, not in the mod or their installation, and Retry is the obviously-correct action. The proactive store verification with repair is better still — it converts a failure-at-launch into a self-healing background task. The naming defect worth fixing in ours: the user-facing string says "corrupt" while the developer-facing string names the wrong hash algorithm, which will mislead whoever debugs it.

---

## LAUNCH-16 — No internet
**Type:** indicator
**Nav path:** Download manager, "Needs attention"; plus the titlebar "Offline" chip; plus "Didn't finish" variants.
**Evidence:** [SOURCE `packages/app-lib/src/error.rs:102-108`; `packages/app-lib/src/install/runner.rs:1842-1859`; `apps/app-frontend/src/components/ui/download-manager/use-install-job-display.ts:333-339`; `apps/app-frontend/src/components/ui/AppActionBar.vue:15-18`, `:283-294`]
**Purpose:** Distinguish "you are offline" from "the download failed", because they need different reactions.
**Operation:** network-shaped failures — `FetchError`, `ApiIsDownError`, `WSError`, `WSClosedError`, `Ratelimited` — all become `network_error`, plus a catch-all that maps any `Error` during a download-ish phase to `network_error` too. The sentence is **"Download couldn't finish"**, narrowed to **"Couldn't download pack"** during `downloading_pack_file`. Two distinct server-side conditions get their own Display strings rather than a generic one: `ApiIsDownError` renders **"Too many API errors, try again in {n} minutes"** and `Ratelimited` renders **"Too many requests, retry in {x} seconds"** (with a pluralising helper). Separately, the titlebar shows a persistent **"Offline"** chip driven by `navigator.onLine` plus `online`/`offline` events.
**Controls:** Retry.
**Persistence:** n/a.
**Rationale & notes:** The pluralised "retry in 3 minutes and 20 seconds" is a small thing that does real work — it converts an opaque failure into a schedule. The catch-all `Any(_)` → `network_error` rule is pragmatic and mostly right, and occasionally wrong in a way that would confuse (a panic during `preparing_java` is reported as a network problem). The Offline chip is a known false positive in a Tauri webview — see OQ-004, where the chip reads "Offline" while the app is demonstrably fetching live data. Two consumers of connectivity disagree inside one app, and neither is authoritative. **For our launcher: derive connectivity from the layer that actually performs the requests, not from a browser-level guess.**

---

## LAUNCH-17 — No loader version selected
**Type:** dialog
**Nav path:** appears over the instance page when a launch fails with "No loader version selected for {loader}".
**Evidence:** [SOURCE `packages/app-frontend/src/components/ui/ErrorModal.vue:85-89`, `:275-283`, `:142-152`; `packages/app-lib/src/launcher/mod.rs:931-937`]
**Purpose:** This is the template for how a launch failure should be handled, and the only one in the app that gets it fully right.
**Operation:** `ErrorModal` pattern-matches the message "No loader version selected for", sets title **"No loader selected"**, remembers the instance id from the launch context, and renders: *"The Modrinth App failed to find the loader version for this instance. To resolve this, you need to repair the instance. Click the button below to do so."* — with a working **"Repair instance"** button that calls `install_existing_instance(id, false)` and closes the modal on success. The row is also the only instance-scoped error type, so it gets a second action surface: the instance page's Play button itself becomes "Repair" (LAUNCH-1 branch 4), which routes to the same install job.
**Controls:** Repair instance; Get support; Close; Debug information (collapsible, copyable).
**Persistence:** n/a.
**Rationale & notes:** This is the one to copy. It names the specific problem, it says what will fix it, it *is* the fix, and it needs no extra context from the user. It is also the only error type where the app recognises "this is a known problem with a known remedy" and acts on that knowledge instead of showing a raw message. The surrounding structure helps too — the error carries the instance id, so the dialog is scoped to the instance the user was looking at, and the same problem is reachable proactively from the Play button rather than only reactively. Compare with LAUNCH-12's flat sentence list: the taxonomy handles *diagnosis*, and this handles *recovery*. A launcher needs both, and this app only has the first at scale.

---

## LAUNCH-18 — Missing file at launch → silent auto-repair
**Type:** state
**Nav path:** invisible — happens inside the launch.
**Evidence:** [SOURCE `packages/app-lib/src/launcher/mod.rs:1035-1053`; `packages/app-lib/src/launcher/download.rs:222-264`]
**Purpose:** A user's install can outlive the shared runtime cache (a cache clear, a failed cleanup, a second app version). The game should still start.
**Operation:** before spawning, `missing_runtime_file` checks two things: the version jar exists, and every classpath-eligible library (respecting `rules`, `include_in_classpath`, and skipping natives) exists in the shared libraries directory. If any is missing, the runtime lock is **dropped**, a full `install_existing_instance` job is started, the launch **blocks** on `wait_for_job`, the lock is re-acquired, and the launch continues. The only trace is an `info!` line: "Restoring missing Minecraft runtime files before launch". The user sees the Download manager working, not an error.
**Controls:** none.
**Persistence:** n/a.
**Rationale & notes:** The right call, and the blocking is what makes it correct — launching against a partially restored cache would produce a class-not-found crash far more confusing than a wait. What is missing is any honesty about it: a user with a persistently failing disk will experience a launch that hangs for minutes with no explanation, because the only signal is a titlebar download row they were not watching. A single sentence at the instance level — "restoring files before launch" — would cost nothing.

---

## LAUNCH-19 — Pre-launch, wrapper and post-exit hooks
**Type:** —
**Nav path:** Settings → (Launch hooks) globally; per-instance settings → a hooks tab, "Custom game launch hooks".
**Evidence:** [SOURCE `packages/app-lib/src/launcher/hooks.rs:1-118`; `packages/app-lib/src/api/instance/run.rs:106-236`; `packages/app-lib/src/launcher/mod.rs:995-1014`, `:1003-1023`; `packages/app-frontend/src/locales/en-US/index.json:4145-4198`]
**Purpose:** Let advanced users run anything around the game without the launcher growing features for it.
**Operation:** three hooks, each settable globally and overridable per instance (the instance value wins if non-empty):

- **Pre-launch** — expanded and run **before** the game, in the instance directory, with the injected env; the launcher **waits** for it, and a non-zero exit aborts the launch with "Non-zero exit code for pre-launch hook: {code}". A hook that cannot be `shlex`-split aborts with "Invalid pre-launch command: {cmd}". This is a genuine gate: the hook can veto a launch.
- **Wrapper** — prefixed to the Java invocation, so the launcher runs `wrapper java <all jvm args>`. Empty after expansion is filtered out; an empty first token is "Empty wrapper command".
- **Post-exit** — spawned after a *successful* exit, in the instance directory, fire-and-forget. A malformed one is "Invalid post-exit command: {cmd}".

**Injected environment variables** (the hook environment, also applied to the game process when any hook is configured): `INST_NAME`, `INST_ID`, `INST_DIR`, `INST_MC_DIR` (an alias for `INST_DIR`), `INST_JAVA`, `INST_JAVA_ARGS`, plus every `custom_env_vars` entry, layered over the process's own environment for *expansion* purposes. `INST_JAVA_ARGS` is computed the same way the real JVM args are: `-Xmx{maximum}M`, then the user's extra Java args, then `--add-opens=java.base/java.lang.reflect=ALL-UNNAMED` on Java 9+, then `--add-opens=jdk.internal/jdk.internal.misc=ALL-UNNAMED` on Java 25+. Unknown `$VAR` references are left verbatim rather than blanked, which is the safer failure mode. The documented variable list is user-facing copy in the locale bundle, one sentence each, e.g. "$INST_DIR: The absolute path to the instance's folder", "$INST_MC_DIR: An alias for $INST_DIR".
**Controls:** three text fields per scope; empty means unset.
**Persistence:** `hook_pre_launch`, `hook_wrapper`, `hook_post_exit` in the settings table, mirrored into instance launch overrides.
**Rationale & notes:** Getting `INST_JAVA_ARGS` *exactly* right — by generating it from the same code path the real launch uses — is the kind of detail that makes hooks trustworthy; a wrapper that receives stale arguments is worse than no wrapper at all. Three separate scopes (global default, per instance, and the expansion environment) with clear precedence is right. The weakness is discoverability: hooks are three text boxes with no validation until launch, and a typo in `shlex`-parseable-but-nonexistent binary produces an OS error at launch rather than a field-level warning.

---

## LAUNCH-20 — Arguments and environment injected into the game
**Type:** —
**Nav path:** n/a.
**Evidence:** [SOURCE `packages/app-lib/src/launcher/args.rs:26-253`, `:256-360`; `packages/app-lib/src/launcher/mod.rs:1103-1176`; `packages/app-lib/src/launcher/hooks.rs:95-118`]
**Purpose:** Assemble the JVM and game command lines correctly per game version — including old versions with no `arguments` block, and new ones with log4j.
**Operation:** the JVM argv, in order: the version manifest's `jvm` arguments with `${natives_directory}`, `${library_directory}`, `${classpath_separator}`, `${launcher_name}`, `${launcher_version}`, `${version_name}` and `${classpath}` substituted (whitespace stripped from each token); for pre-1.13 versions with no `arguments`, the legacy pair `-Djava.library.path=…` and `-cp …`; then `-Xmx{maximum}M`; then the log4j `-Dlog4j.configurationFile=…` if the version declares logging; then `-javaagent:{theseus.jar}`; then `-Dmodrinth.internal.ipc.host={ip}` and `-Dmodrinth.internal.ipc.port={port}`; then `-Dmodrinth.internal.quickPlay.serverVersion={…}`, plus `…quickPlay.host`/`…quickPlay.port` when the game supports injected quick play; then the user's extra Java arguments. Afterwards the launcher appends `--add-opens=java.base/java.lang.reflect=ALL-UNNAMED` on Java 9+ and `--add-opens=jdk.internal/jdk.internal.misc=ALL-UNNAMED` on Java 25+ — the latter explicitly for JEP 512.

The main class is `com.modrinth.theseus.MinecraftLaunch` (from `theseus.jar`, extracted to a `TempDir` kept alive for the process's lifetime), with the version's `mainClass` as its first argument, then the game's arguments with `${accessToken}`/`${auth_access_token}`/`${auth_session}` → the Minecraft access token, `${auth_player_name}` → username, `${auth_uuid}`/`${uuid}` → the UUID without dashes, `${auth_xuid}` → `0`, `${clientid}` → a fixed UUID, `${user_properties}` → `{}`, `${user_type}` → `msa`, plus paths and version names. On game versions too old for `--server`/`--port` quick play, the server address is appended as those legacy flags instead.

Two environment details worth copying: `_JAVA_OPTIONS` is **removed** from the game process's environment (its presence would override the user's carefully-chosen Java arguments), and on macOS `DYLD_FALLBACK_LIBRARY_PATH` is removed during dev builds because Cargo sets it and it breaks the game.
**Controls:** memory (max MB), window size, fullscreen (written into `options.txt`), extra Java arguments, custom environment variables, wrapper.
**Persistence:** `extra_launch_args`, `custom_env_vars`, `mc_memory_max`, `mc_force_fullscreen`, `mc_game_resolution_{x,y}` in settings, each overridable per instance.
**Rationale & notes:** The two `--add-opens` flags are correct modern practice and are usually missed by third-party launchers, so a JDK 17+ or 25+ install works here where it might not elsewhere. Stripping `_JAVA_OPTIONS` is a genuinely thoughtful safety measure that protects the user's JVM arguments from an ambient environment variable they probably forgot about. Injecting the IPC host/port and a javaagent for the modrinth.jar main class is how the launcher gets in-process hooks (server-join detection, in-game Modrinth integration) — a design our launcher may or may not need, but the *pattern* of a launcher-side agent plus a local socket is worth understanding before we design our own runtime integration.

---

## LAUNCH-21 — Live observation of any launch state
**Type:** state
**Nav path:** every LAUNCH item above.
**Evidence:** [ORCH-PENDING] — no game was ever launched. The brief forbids using any real Microsoft/Minecraft/Modrinth account, and `run()` refuses without credentials (`run.rs:55-57`), so nothing past the Play button was executed. Everything above is `SOURCE`. What the orchestrator **can** observe unauthenticated, and should:
- the Play button in its `Repair` state (any instance that is not `installed` — a brand-new instance reaches this immediately, no account needed);
- the Play button's `Starting...` state, which is reachable but very brief and needs the backend to be slow;
- pressing Play with no account → the "Minecraft required" modal (from module 07, AUTH-10);
- the Download manager's empty state ("No installation tasks") and its bar;
- an install job end-to-end — **this is fully observable without any account**, since creating an instance and installing a public modpack needs only the public content API. That gives real observation of LAUNCH-4 and LAUNCH-5: the phase labels, the progress bar, the byte/count formatting, and — by filling the target filesystem — LAUNCH-14's "Couldn't save files" and LAUNCH-15's "Downloaded file is corrupt";
- the generic error modal's `no_loader_version` panel (LAUNCH-17), if a launch can be provoked far enough to produce that error without credentials — doubtful, since the credentials check runs before it in `run()`.
**Purpose:** n/a
**Operation:** n/a
**Controls:** n/a
**Persistence:** n/a
**Rationale & notes:** The observation gap is concentrated in exactly the states the brief called out as interesting — playing, exited, crashed — because they all require an authenticated, licensed account. The highest-value observation available without one is the **install job pipeline**, which shares its progress reporting, error taxonomy and copy with the launch pipeline. That is enough to check the naming analysis against reality. What it cannot check is the exit-status recording, the post-exit sequence, and the mclo.gs crash analysis; those will have to rest on the source reading above, which is why they are marked as reading rather than observation.

---

## Cross-module summary for the implementer

**The shape to copy.** A persistent, phase-named task surface (LAUNCH-4/5) rather than a transient modal progress bar; a two-stage error vocabulary of code plus sentence, with the phase disambiguating (LAUNCH-12); a "Copy details" that produces a pre-written, already-censored support document (LAUNCH-12); and at least one error type that is not merely described but *fixed* (LAUNCH-17). Modrinth has the last one exactly once, which tells you how much headroom there is.

**The shape to consciously improve on.** No crash surface at the moment it matters (LAUNCH-07/11 — the best tool fires only if you happen to be looking). No graceful stop (LAUNCH-09). "Couldn't save files" for a full disk, in an app that already knew (LAUNCH-14). Two competing progress systems (LAUNCH-06). A connectivity indicator that can be wrong while sitting next to a correct one (LAUNCH-16, OQ-004). Silent auto-repair with no user-facing acknowledgement (LAUNCH-18). And an entirely flat treatment of everything after "the process started", which is where the mission's interest actually was.

**The vocabulary worth stealing verbatim-as-concept** (not as copy): phase names in plain language; error sentences that describe an outcome rather than a mechanism; a permission sniff layered over a generic filesystem code; a pre-flight space check with a working margin; `SIGKILL` as an honest, fast stop with no pretence of graceful shutdown; a bounded queue for downloads and no bound at all for the game.