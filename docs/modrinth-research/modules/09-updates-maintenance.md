# 09 — Updates and maintenance

## Coverage table

| ID | label | type | evidence | status |
| --- | --- | --- | --- | --- |
| UPD-1 | App self-update | process | `SOURCE apps/app/src/main.rs:19-22,78-85`; `settings.rs:83-85` | documented |
| UPD-2 | Update preference tri-state | setting | `SOURCE settings.rs:85` | documented |
| UPD-3 | Skipped / deferred update | setting | `SOURCE settings.rs:83-84`; `state/db.rs:86-89` | documented |
| UPD-4 | Per-instance content updates | process | `SOURCE state/instance_content_update_checks` | documented |
| UPD-5 | Update check scheduling | process | `SOURCE` install modules | partial |
| UPD-6 | Repair / reinstall | action | `SOURCE` install modules | partial |

## Second observation pass (2026-10-14)

The first pass could not run a job or open an instance, so the whole update and
maintenance surface was `SOURCE`/`partial`. A second pass created and populated an
instance with no account (OQ-012) and observed:

- **Task manager** `[S-0125]`. The titlebar download icon opens a **"Tasks"**
  popover: a `Complete` group listing each finished job with its age and type
  ("Fabulously Optimized · 4m ago · Modpack", "Fabric 26.3 · 10m ago · New
  instance"), a per-task delete, and `Clear all`. This is the user-facing face of
  the install/update pipeline module 03 describes. **UPD-5/UPD-6 become
  `documented`/`OBSERVED` for the running-job and completed-job states.**
- **The running job** `[S-0097]`. A live job shows as a titlebar chip with its
  phase and throughput (e.g. "Fabric 26.3 2.21 MB/s"), and on the instance as an
  `Installing…` button in place of `Play`. There is no separate progress window.
- **Repair** `[S-0106]`. Observed in the instance's Installation page; copy as
  quoted in module 05.
- **Per-instance content updates** `[S-0117]`. The instance Content table carries a
  per-row update/reinstall action; there was no newer version during the pass, so
  the "update all" affordance and the update-in-progress state remain unobserved
  (still `[ORCH-PENDING]` where module 05 lists them).
| UPD-7 | Cache management | action | `OBSERVED` `cache` table | partial |

---

## UPD-1 — App self-update

**Type:** process **Nav path:** background; surfaced via a toast and About
**Evidence:** `SOURCE apps/app/src/main.rs:19-22,78-85` · `OBSERVED S-0008` (the
version in the settings footer)

**Purpose.** Keeps the launcher current without the user having to think about it,
and — importantly — gives the app a way to ship fixes for the update mechanism
itself.

**Operation.** The updater is compiled behind a **cargo feature flag**, with a
no-op implementation selected when the flag is off:

```rust
#[cfg(feature = "updater")]
mod updater_impl;
#[cfg(not(feature = "updater"))]
mod updater_impl_noop;
```
`SOURCE apps/app/src/main.rs:19-22`, re-exported at `:78-85`

The preference is tri-state (UPD-2), and the settings row also carries two
companion fields — `skipped_update` and `pending_update_toast_for_version` —
which are cleared when read (`SOURCE packages/app-lib/src/state/db.rs:86-89`).

**Rationale & notes.** The feature flag is the transferable idea: the updater
can be compiled out entirely for platforms or builds that do not want it, and
the no-op keeps every call site valid. Ours should do the same rather than
sprinkling `#[cfg]` around call sites.

## UPD-2 — Update preference tri-state

**Type:** setting (`Option<bool>`)
**Evidence:** `SOURCE packages/app-lib/src/state/settings.rs:85`

**Purpose.** Decides whether updates install themselves.

**Controls:** three states, not two:

| Stored | Meaning |
| --- | --- |
| `null` | **ask** — the honest default |
| `true` | auto-install |
| `false` | never |

**Rationale & notes.** A checkbox cannot express "ask me": `false` would have to
mean either "never" or "ask", and they are different promises. The `Option<bool>`
makes the default the non-committal one, which is what a new install should be.

## UPD-3 — Skipped and deferred updates

**Type:** settings
**Evidence:** `SOURCE settings.rs:83-84` · `state/db.rs:86-89`

**Purpose.** Remembers that the user said "not now" so the app does not ask
again for the same version, while still noticing a newer one.

**Operation.** `skipped_update` holds a deferred version;
`pending_update_toast_for_version` holds one to nudge about. **Both are cleared on
read** (`state/db.rs:86-89`) — a one-shot, not a persistent flag. That is the
right shape: a nudge that fires once and then forgets itself.

**Rationale & notes.** One-shot, self-clearing notifications are a pattern we
should adopt wherever we might otherwise nag. Any "you should know this" that
does not erase itself after being shown will eventually be trained out by the
user.

## UPD-4 — Per-instance content updates

**Type:** process
**Evidence:** `OBSERVED` table `instance_content_update_checks` · `SOURCE`
instance content modules

**Purpose.** Tells the user that a mod in one of their instances has a newer
version, and offers the update.

**Operation.** The check is stored per instance per content item, which is why
the table exists rather than a boolean on the instance: an instance with fifty
mods needs to know which of them are stale. `[ORCH-PENDING]` — the comparison
logic (local version id vs. remote latest for the profile's loader and game
version) was not traced line by line.

**Rationale & notes.** Storing the *result* of the check rather than
recomputing it is what makes "you have 3 updates" cheap to render. Our
`palantir-loader` computes update candidates on demand today; caching the answer
per instance is the improvement.

## UPD-5 — Update check scheduling

**Evidence:** `SOURCE` install modules

**Status:** partial — the intervals and triggers were not pinned down in this
mission. `[ORCH-PENDING]`.

## UPD-6 — Repair / reinstall

**Evidence:** `SOURCE` install modules

**Status:** partial — the affordances exist in the instance action set (see
`modules/04`), but what a repair actually re-downloads versus verifies was not
traced.

**Rationale & notes.** The value of a repair is that it is *safe*: it should
never delete anything the user added by hand. Whatever we build should start
from "verify hashes, re-fetch mismatches" rather than "delete and re-install".

## UPD-7 — Cache management

**Type:** action
**Evidence:** `OBSERVED` — the `cache` table held 601 rows on a fresh install

**Purpose.** Lets a user reclaim space and fix a corrupted cache.

**Operation.** `[ORCH-PENDING]`. The disk layout in `data-model.md` is what
makes this safe: `CacheStorage`, `WebKitCache`, `store/staging` and
`localstorage` are disposable, while `app.db` and `profiles/` are not. An app
that can say which is which can offer "clear cache" without offering "delete my
stuff".

---

## Scope note

**Servers and hosting are out of our scope** and are not covered here. For
calibration on what that costs the reference: the surface spans five routes
(`/hosting/manage/` plus Overview, Content, Files, Backups, Access —
`SOURCE routes.js:14-49`), three feature flags (`ServersInApp`, `ServerRamAsBytesAlwaysOn`,
`ServerProjectQa` — `SOURCE settings.rs:96-107`), the `synced_servers` and
`synced_server_state` tables, the `server_join_log` table and the
`server-ping` API module. `OBSERVED S-0006` shows the surface working, including
an invite flow with Accept/Decline. We decline all of it deliberately.