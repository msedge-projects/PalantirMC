# Data model — what the Modrinth App persists, and where

Read from a **running instance**: the on-disk tree and the `app.db` schema and
live values were read directly, not inferred. Source citations are against
`modrinth/code` at tag `v0.21.9`, commit `6ebd75b6`.

Paths below are **Linux**, this mission's platform. Modrinth ships Windows and
macOS builds; the app identifier is `ModrinthApp` (`SOURCE apps/app/tauri.conf.json`),
and each platform's data root is its own convention. Our rewrite ships Windows,
so the `APPDATA` equivalent is the mapping to make — not a copy of the Linux
path.

---

## 1 — Directory layout

`OBSERVED` on a running install:

```
~/.local/share/ModrinthApp/        data root  (== settings.custom_dir)
├── app.db                         SQLite: settings, instances, cache, accounts
├── app.db-wal / app.db-shm        write-ahead log + shared-memory index
├── hsts-storage.sqlite            WebKit HSTS (webview-managed)
├── profiles/                      one directory per profile
├── store/                         content store: content/ + staging/
├── synced-options/                hotbars/, servers/
├── databases/indexeddb/           webview storage
├── storage/                       webview local storage (one opaque dir/hash)
├── localstorage/  mediakeys/      webview-managed
├── CacheStorage/  WebKitCache/    webview-managed, disposable
├── cookies                        webview cookie jar
└── launcher_logs/
    └── session_YYYYMMDD_HHMMSS.log
```

Two things an implementer should copy:

- **Logs are per session, timestamped, never rotated over.** One file per run,
  named for when it started. The oldest of the files above is from 2026-10-01;
  a launcher that ran that long and kept every session log is not truncating.
  Ours currently writes `*.log` files that `AGENTS.md` treats as build output and
  ignores — a launcher log that is git-ignored is fine, but the user must still
  be able to *find* it, which the About page exposes (`SOURCE settings.rs`
  region; roadmap slice 8).
- **Cache and storage are separable from state.** `CacheStorage`, `WebKitCache`,
  `store/staging` and `localstorage` are disposable; `app.db` and `profiles/`
  are not. Being able to say which is which is what makes a "clear cache" action
  safe to offer.

---

## 2 — `app.db`

`OBSERVED`: **59 tables**, 49 applied migrations, 6.4 MB on a fresh install.

### The tables that matter

| Table | Rows (live) | Holds |
| --- | --- | --- |
| `settings` | **1** | every global setting, `id = 0` |
| `sync_feature_settings` | 7 | per-feature sync state |
| `app_metadata` | 5 | install/version metadata |
| `instances` | 0 | one row per instance |
| `instance_groups` | 1 | library grouping |
| `instance_content_entries` | 0 | mods/resources/shaders/datapacks per instance |
| `instance_content_sets` | 0 | content-set versioning |
| `instance_content_update_checks` | 0 | which content has an update |
| `instance_launch_overrides` | 0 | per-instance setting overrides |
| `instance_files` | 0 | the file index (a DB, not a disk walk) |
| `store_operations` / `store_blobs` | 0 | content store operations and blobs |
| `java_versions` | 0 | discovered/installed JDKs |
| `minecraft_users` / `minecraft_device_tokens` | 0 | Microsoft accounts + tokens |
| `modrinth_users` | 0 | Modrinth accounts |
| `processes` | 0 | running instances |
| `onboarding_checklist` | 1 | the "Getting started" panel |
| `cache` | 601 | API response cache |
| `joined`/`join_log` | 0 | joined servers |
| `synced_game_option_*` (5 tables) | 0 | Minecraft game options, synced |
| `synced_hotbar_state` | 0 | synced hotbars |

`SOURCE packages/app-lib/src/state/settings.rs:112` for the settings table;
the instance/content tables are the `instance_content_*` family in the same
crate.

### The decision worth taking

**The instance file index is a table, not a directory walk.** `instance_files`
exists, and so do `store_operations`, `store_blobs`, `store_retained_refs` and
`store_instance_files`. Content is a **store** with operations, blobs and
reference counting, staged separately from committed. `store/staging/` on disk
matches the `store_operations` table.

This is the single largest architectural difference from a conventional launcher
and it is what makes content **update** and **sync** possible: the app knows
which blobs an instance is using, so it can compute a diff, share unchanged
files between instances, and roll a change back. A launcher that copies jars
into each instance directory cannot do any of that.

**Can we avoid the database?** Partly, and the honest answer is: not if we want
update checks, content sharing or sync. The `settings` table alone would be
fine as a JSON file — but note that even Modrinth does *not* use a JSON file for
settings, they use a table (`SOURCE settings.rs:112`, `OBSERVED`). Our `store`
tables already have real content-store analogues to model, which suggests the
DB is a reasonable shape for us rather than a Modrinth peculiarity.

---

## 3 — Settings row

`OBSERVED` from the live row. **One row, `id = 0`, checked by
`CHECK (id = 0)`.**

| Key | Live | Schema default | Group |
| --- | --- | --- | --- |
| `theme` | `dark` | `dark` | appearance |
| `locale` | `en-US` | `en-US` | appearance |
| `default_page` | `home` | `home` | behaviour |
| `mc_memory_max` | **`4096`** | **`2048`** | launch |
| `mc_game_resolution_x` / `_y` | `854` / `480` | `854` / `480` | launch |
| `mc_force_fullscreen` | 0 | `FALSE` | launch |
| `extra_launch_args` | `[]` | — | launch |
| `custom_env_vars` | `[]` | — | launch |
| `hook_pre_launch` / `hook_wrapper` / `hook_post_exit` | `NULL` | `NULL` | launch |
| `max_concurrent_downloads` | `10` | `10` | network |
| `max_concurrent_writes` | `10` | `10` | network |
| `telemetry` / `discord_rpc` / `personalized_ads` | 1 | `TRUE` | privacy |
| `developer_mode` | 0 | `FALSE` | advanced |
| `custom_dir` | `/root/.local/share/ModrinthApp` | `NULL` | advanced |
| `prev_custom_dir` | same | `NULL` | advanced |
| `version` | **`3`** | `1` | meta |
| `skipped_update` / `pending_update_toast_for_version` | `NULL` | `NULL` | updates |
| `auto_download_updates` | `NULL` | `NULL` | updates |
| `feature_flags` | `{}` | `{}` | meta |
| `show_files_tab_in_instances` | 1 | `TRUE` | features |
| `show_worlds_tab_in_instances` | 1 | `TRUE` | features |
| `show_screenshots_tab_in_instances` | 0 | `FALSE` | features |

Two values where **live ≠ schema default**, and both are explained:

- `mc_memory_max` is `4096` against a `2048` default. Migration `2 → 3` rewrites
  it: *"Update old default memory setting from 2GB to 4GB (depending on system
  memory)"* — `SOURCE packages/app-lib/src/state/settings.rs:425-432`, using
  `crate::api::jre::default_memory_max_mb()`. Gated on the value still equalling
  `LEGACY_DEFAULT_MEMORY_MB`, so a user who deliberately chose 2 GB keeps it.
  **This is the pattern to copy for every default we ever change.**
- `version` is `3` against `DEFAULT 1`, because `Settings::migrate` advanced it
  through the ladder to `CURRENT_VERSION = 3` (`SOURCE settings.rs:110, 366`).

**Modrinth-specific vs standard practice** — the distinction that tells our
implementer what to copy:

| Modrinth-specific | Standard launcher practice |
| --- | --- |
| `synced_game_option_*` (5 tables), `synced_hotbar_state` | instance/profile directory layout |
| `store_operations` / `store_blobs` / `store_retained_refs` | game version / loader bookkeeping |
| `instance_content_sets` + `_update_checks` (versioned content sets) | Java discovery |
| `personalized_ads`, `telemetry`, `discord_rpc` | memory / resolution settings |
| `feature_flags` (servers, QA, i18n debug) | launch args |
| `sync_feature_settings` (cloud sync scaffolding) | hooks |
| `onboarding_checklist` | logging |
| Modrinth account tables (`modrinth_users`) | crash handling |
| content store sharing between instances | |

Roughly half each. **The content store and the versioned content sets are the
parts worth stealing** — they are what make updates and sync tractable, and they
are not Modrinth branding but launcher engineering. The accounts, ads and
sync scaffolding we decline along with the rest of our scope.

---

## 4 — Instance format

`SOURCE packages/app-lib/src/state/instance_types.rs` and the instance modules
under `packages/app-lib/src/state/instances/`.

A profile is a container (`profiles/`) owning instances; an instance carries its
game version, loader kind and loader version, its content entries (each with a
project id, version id, and whether it is enabled), its launch overrides, and a
file index. Content is **referenced by Modrinth project/version ids**, not by
file path — which is what lets the app resolve an update without re-reading the
directory.

**Second observation pass (2026-10-14): it is not JSON — the instance model is a
second set of SQLite tables in the same `app.db` as settings.** Read live from two
real instances (`OBSERVED`; the first pass had none, which is why this was
`[ORCH-PENDING]`). The tables and their key columns:

| Table | Cols | Key columns | What it holds |
| --- | --- | --- | --- |
| `instances` | 13 | `id` (`local:<uuid>`), `path` (profile-folder name), `applied_content_set_id`, `install_stage`, `launcher_feature_version`, `update_channel`, `name`, `icon_path`, `created`, `modified`, `last_played`, `submitted_time_played`, `recent_time_played` | the instance itself |
| `instance_content_sets` | 11 | `id` (`content-set:<uuid>`), `instance_id`, `name`, `source_kind`, `status`, `game_version`, `protocol_version`, `loader`, `loader_version` | one row per game-version + loader pair, so an instance can carry more than one |
| `instance_content_entries` | 13 | `id`, `instance_id`, `content_set_id`, `file_id`, `project_type`, `project_id`, `version_id`, `source_kind`, `server_requirement`, `client_requirement`, `enabled`, `added_at`, `modified_at` | installed content, keyed by **Modrinth project/version id**, with per-side requirement flags |
| `instance_files` | 10 | `id`, `instance_id`, `relative_path`, `file_name`, `enabled`, `sha1`, `size`, `missing`, `added_at`, `modified_at` | the on-disk file index (path + sha1) |
| `instance_launch_overrides` | 2 | `instance_id`, `overrides` (msgpack blob) | per-instance launch overrides; the blob also carries `hooks` and the `visible_tabs` map |
| `instance_icon_configs` | 3 | `instance_id`, `background` (JSON gradient), `symbol` | the icon builder's state |
| `instance_groups` / `instance_group_memberships` | 3 / 2 | `id`,`name`,`display_order`; `instance_id`,`group_id` | `Favorites` is seeded as `group:favorites` |
| `instance_links` | 19 | `instance_id`, `link_kind`, then Modrinth / server / shared / imported columns | one wide table unifying how each instance is linked outward |

A live example makes the id-keying concrete: instance `local:418a5ccc…` ("Fabric
26.3") has content set `content-set:af71a61e…` with `game_version = 26.3`,
`protocol_version = 777`, `loader = fabric`, `loader_version = 0.19.5`; its content
entry for Fabric API is `project_id = P7dR8mSH`, `version_id = bq3SsNGk`; and its
file index row is `mods/fabric-api-0.163.0+26.3.jar` with a sha1 and size.

Other instance-scoped tables present on the same install: `instance_servers`,
`instance_server_pack_state`, `instance_server_projection_entries`,
`instance_sync_*`, `instance_game_option_*`, `synced_game_option_*`, `install_jobs`,
`processes`, `screenshots`, `attached_world_data`, `join_log`, plus the account
tables `minecraft_users`, `modrinth_users`, `minecraft_device_tokens`.

`[ORCH-PENDING]` (smaller now) — the exact `instance_types.rs` `serde` field names
and the msgpack layout inside `overrides` were not decoded field by field. Read
`instance_types.rs` before writing the serializer, but treat the table schema
above as the storage contract: the instance model is relational, not a document.

---

## 5 — Cache

`cache` holds 601 rows on a fresh install with no instances, so it is
almost entirely API response caching — search results, project metadata, version
lists. `synced-options/` holds Minecraft game options and hotbars that the app
mirrors out of the instance directories so it can sync them.

## 6 — Logging

`launcher_logs/session_YYYYMMDD_HHMMSS.log` — one file per run. `SOURCE
packages/app-lib/src/logger.rs:27` sets `theseus=info,theseus_gui=info,webview=trace`
by default, and `:81` documents `RUST_LOG=theseus=trace` for everything. The
filter is env-driven, which is how a user is meant to produce a useful bug
report without a debug build.