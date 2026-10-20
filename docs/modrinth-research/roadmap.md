# Roadmap — closing the Modrinth gap in the rewrite

The gap analysis (`gap-analysis.md`) says *what* is missing. This says what to
build, in what order, and how to know each slice is done.

**Ordering rule** (mission §6): (a) what the owner called out — tabs, settings,
functions — then (b) daily-use frequency, then (c) effort. Effort is deliberately
last: the top ten gaps are all settings, and several of them are small once the
settings store exists.

Every slice is independently shippable and carries acceptance criteria written
as **user-visible behaviour X, verified by Y**. "Y" is always something that can
be checked without asking anyone's opinion.

**Standing constraints for every slice.** Rust + iced 0.12; no `unwrap`/`expect`
outside tests; `cargo test --workspace --all-targets --locked` and
`cargo clippy --workspace --all-targets --locked -- -D clippy::correctness` pass;
push to both remotes and confirm the public run green (`AGENTS.md` §§1,3).
Clean-room: no Modrinth code, assets or branding — behaviour only, in our own
copper/matte language.

---

## Slice 0 — A settings store with schema-owned defaults

**Why first.** Everything in slices 1–4 hangs off this. Without it, each setting
is a hand-placed widget plus a bespoke save path, and the count of "missing
settings rows" only goes up.

**Do.** One serialisable `Settings` struct owned by `palantir-core`, with the
default for every field declared in the default implementation — the way
Modrinth's `CREATE TABLE settings` owns its defaults (`OBSERVED`, `modules/06`).
One load, one save, no partial paths. Include a `version` field and a
`migrate(from)` function so a later default change can move existing users.

**Acceptance criteria**
- A single `Settings::default()` is the only place a default is written; no
  widget invents one.
- Adding a field compiles with no other edit (defaults, not a required arg).
- A round-trip through the store preserves every field (a test, not a
  inspection).
- `settings.version` exists and is asserted at load.

---

## Slice 1 — The Features section (owner's #1 complaint)

**Why second.** It is the cheapest visible win and it is literally a settings
section that controls our own UI, which we have no equivalent of
(`SOURCE settings.rs:26-33`).

**Do.** A `Features` settings tab with toggles that hide instance tabs. Start
with what we actually have: `Overview`, `Content`, `Worlds`, `Logs`, `Manage`.
Design the key names for tabs we will *add* (`Files`, `Screenshots`) so they are
additive later.

**Acceptance criteria**
- Each toggle hides its tab and the choice survives a restart.
- A tab hidden here does not appear anywhere, including deep links.
- Turning a toggle off and on again restores the previous state exactly.
- Default-on for the five existing tabs, so the default install looks unchanged.

---

## Slice 2 — Appearance: real theme options and the honesty rule

**Do.** Extend `palantir-theme` beyond the current accent/scale with a theme
selector offering the options we can actually render, plus OLED, plus
sync-with-system if the OS value is reachable on our two target platforms.

The Modrinth detail worth copying is the **risk-on-the-row** pattern: advanced
rendering states, in the row itself, that it may cause issues and needs a
restart (`OBSERVED S-0020+`). And the detail worth *not* copying is their
connectivity indicator, which is wrong (`App.vue:465`, `OBSERVED S-0003`).

**Acceptance criteria**
- Every theme is reachable from Settings and applies without a restart, except
  any that genuinely do — and those say so on the row, before the user clicks.
- A restart-requiring change names the consequence on the control itself.
- No setting claims a capability we do not have.

---

## Slice 3 — Launch arguments, environment variables, hooks

**Why.** The largest functional hole after settings, and the one users ask for
when our launch fails for a reason the UI cannot express.

**Do.** In `palantir-loader`: `extra_launch_args: Vec<String>`,
`custom_env_vars: Vec<(String, String)>`, and the three hooks. Two details from
Modrinth's source are worth copying deliberately:
- `custom_env_vars` is stored as an **ordered array of pairs**, not a map
  (`SOURCE settings.rs:59`) — argument order matters.
- Hooks parse with a real shell quoter, and the v1→v2 migration exists only
  because they used to split on whitespace and broke paths with spaces
  (`SOURCE settings.rs:397-420`). Ours should start correct and need no
  migration.

**Acceptance criteria**
- An arg set on an instance reaches the spawned process, in order, verified by
  reading the child's environment/argv.
- An env var with an `=` in its value survives intact.
- A pre-launch hook with a space in the path runs (regression test for the
  quoting trap).
- A post-exit hook fires on normal exit and on crash.

---

## Slice 4 — Java installation management

**Do.** List, add, remove Java installations; per-instance override. Our Java tab
exists; management does not. Adopt the global-default-with-override shape
(`SOURCE state/java_globals.rs`).

**Acceptance criteria**
- An added JDK appears in the list and is selectable for a launch.
- A removed JDK is no longer selectable and cannot be chosen by a stale instance
  override (the launch fails with a clear message naming it).
- Per-instance override beats the global default, and clearing the override
  restores the global.

---

## Slice 5 — Data directory with a recoverable move

**Do.** `custom_dir` **plus** `prev_custom_dir` (`SOURCE settings.rs:76-77`).
Keeping the previous value is what makes a directory move safe, and it is one
column.

**Acceptance criteria**
- Changing the directory moves instances and the app still launches.
- If the move fails, the app starts from the old location and says so.
- The previous location is retained and offered as the way back.

---

## Slice 6 — Resizable browse: density, pagination, facets

**Do.** The three browse affordances we lack: a view-density select, real
pagination, and a facet sidebar with "hide already installed"
(`OBSERVED S-0003`). Our search already debounces and our cards already carry
type/author/download metadata (`UI-HANDOFF.md`) — this is the rest of the same
surface.

**Acceptance criteria**
- Density changes the number of cards without a refetch of the same page.
- Pagination reaches the last page and the total is correct (`OBSERVED S-0003`
  showed a real total of 942).
- "Hide already installed" removes projects already present in the library, and
  unchecking restores them.

---

## Slice 7 — App self-update, tri-state

**Do.** `Option<bool>`-shaped preference — ask / auto / never — plus
`skipped_update` and `pending_update_toast_for_version` (`SOURCE settings.rs:83-85`).
Three states, because "ask me" cannot be expressed as the negation of "never".

**Acceptance criteria**
- All three states are reachable and mean what they say.
- Declining does not re-prompt for the same version; a new version prompts.
- A failed update leaves the current version intact and says what failed.

---

## Slice 8 — Developer mode and feature flags

**Do.** A `developer_mode` toggle (`SOURCE settings.rs:80`) and a
**typed** `FeatureFlag` enum rather than a string map (`SOURCE settings.rs:96-107`),
so flags cannot typo, cannot collide, and are greppable. Use it to gate what is
still in progress — that is what the reference does with `ServersInApp`,
`ServerProjectQa` and the rest.

**Acceptance criteria**
- A flag that is off makes its surface unreachable, not merely hidden.
- Adding a flag is a compile error until its UI exists.
- Developer mode exposes the logs folder and version in one place.

---

## Slice 9 — Instance context menus and the destructive-action inventory

**Do.** A complete action set per instance: open, play, edit, duplicate, export,
open folder, delete — with confirmation on delete and a receipt after
(`UI-HANDOFF.md`). Use the reference's library components
(`modules/04`) as the checklist; the frequent complaint in this project area is
a missing context action, so completeness matters more than polish.

**Acceptance criteria**
- Every action has a keyboard-reachable path, not just a right-click.
- Delete requires confirmation and names what will be removed.
- The receipt renders inside the open dialog, not behind the scrim — our existing
  behaviour, which is ahead of the reference.

---

## Slice 10 — Breadcrumb and titlebar route context

**Shipped 2026-10-17** — `crates/palantir-desktop/src/route.rs`, rendered above
every page, with Back/Forward over a trail of visited routes (Alt+Left/Right).
The window smoke reads it in the actual window; see `rewrite/NOTES.md`.

**Do.** A breadcrumb in the titlebar (`OBSERVED S-0002,S-0003`). Cheap, and it is
the main answer to the reference's icon-only rail: if the rail is icons, the
user must always be able to see where they are in words.

**Acceptance criteria**
- Every navigable surface has a breadcrumb, and it matches the route.
- Back/forward update it.

---

## Slice 11 — Per-instance settings override, stated explicitly

**Do.** Make the inheritance rule visible: which settings inherit, which
override, and what clearing an override does. Modrinth keeps global defaults in
one row and overrides per instance (`SOURCE settings.rs:60-74`).

**Acceptance criteria**
- An inherited setting displays as inherited and is visibly not editable at
  instance level, or is editable with the override marked.
- Clearing an override restores the global value and says so.

---

## Deliberately not on this roadmap

With the reason, so nobody re-litigates it:

| Not building | Why |
| --- | --- |
| Skins / cape selector | our stated scope (`OBSERVED S-0004`) |
| Screenshot gallery | our stated scope (`OBSERVED S-0005`) |
| Servers / hosting | our stated scope; the reference spends three feature flags and five routes on it (`SOURCE settings.rs:96-107`, `routes.js:14-49`) |
| Friends / presence | needs an account we do not have |
| Telemetry, personalized ads, Discord RPC | our stated scope; and we would rather ship none of them than a tri-state opt-out |
| "Sync across devices" | needs an account and a backend |
| A **narrow-window layout** | we already have one at 840×600; the reference has none below 1100×700 |

---

## Sequencing

```
0  settings store
├── 1  Features section          ← the owner's complaint, answered
├── 2  Appearance + honesty rule
├── 3  args / env / hooks
├── 4  Java management
├── 5  data directory
├── 6  browse density + pagination + facets
├── 7  app self-update
├── 8  developer mode + flags
└── 9  instance context menus
    10 breadcrumb
    11 per-instance override
```

Slices 1–5 are the settings body of work and depend only on slice 0. Slice 6 is
independent of settings and can run in parallel from day one. Slices 9–11 are
independent of both. **If only five slices ever ship, ship 0, 1, 3, 4 and 9** —
that is the shortest path from "our app is missing settings and functions" to
not being told so again.