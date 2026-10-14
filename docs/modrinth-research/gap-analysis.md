# Gap analysis — Modrinth App vs the PalantirMC 2.0 rewrite

Maps every Modrinth capability found in this study onto **our** implementation,
the rewrite at `/PalantirMC/rewrite/`. Status is one of:

| Status | Meaning |
| --- | --- |
| **Equivalent** | we have it, and it behaves the same way |
| **Partial** | we have the concept; rows/options/behaviour are missing |
| **Missing** | we do not have it |
| **Out-of-scope** | deliberately declined against our stated scope |

**Our stated scope** (from the mission and `UI-HANDOFF.md`): two editions,
Java **and** Bedrock, both managed; **no** skins, **no** screenshots gallery,
**no** servers, **no** hosting, **no** telemetry. Direction is Modrinth's
**UX, not its UI**.

Our current shape, for reference: `Page = {Home, Library, Discover, Settings,
About}` (`rewrite/crates/palantir-desktop/src/main.rs:49`), instance tabs
`{Overview, Content, Worlds, Logs, Manage}` (`main.rs:57`), settings tabs
`{Appearance, Language, Java, Window, Resources, Privacy, Behaviour}`
(`main.rs:88`). ~28.4k lines across `palantir-core`, `palantir-net`,
`palantir-loader`, `palantir-desktop`, `palantir-bedrock`, `palantir-theme`.

---

## 1 — The top ten gaps

Ordered by the mission §11 priority: what the owner called out, then daily-use
frequency, then effort.

| # | Capability | Modrinth evidence | Our status | Target module |
| --- | --- | --- | --- | --- |
| 1 | **46 persisted settings with schema-owned defaults** | `OBSERVED` `settings` table; `SOURCE app-lib/src/state/settings.rs:10` | **Missing** | `palantir-desktop` settings store |
| 2 | **A `Features` section that toggles instance tabs** (Files/Worlds/Screenshots/skin-selector) | `SOURCE settings.rs:26-33` | **Missing** — we have no section for hiding our own UI | `palantir-desktop`, instance tabs |
| 3 | **Pre-launch / wrapper / post-exit hooks** | `SOURCE settings.rs:74,487` | **Missing** | `palantir-loader` |
| 4 | **Extra JVM args + custom environment variables** | `SOURCE settings.rs:58-59` | **Missing** | `palantir-loader` |
| 5 | **Java installation management** (list, add, remove, per-instance override) | `SOURCE state/java_globals.rs`; `java_versions` table | **Partial** — we have a Java tab, not management | `palantir-loader`, `palantir-desktop` |
| 6 | **Per-instance override of global settings** | `SOURCE settings.rs:60-74` vs per-profile overrides | **Partial** — draft/revert exists, override inheritance unclear | `palantir-core` settings |
| 7 | **Custom app data directory with a recoverable move** (`custom_dir` + `prev_custom_dir`) | `SOURCE settings.rs:76-77` | **Missing** | `palantir-core` paths |
| 8 | **App self-update, tri-state** (`null`/`true`/`false`) + skip/defer tracking | `SOURCE settings.rs:83-85` | **Missing** | `palantir-net` |
| 9 | **Developer mode** | `SOURCE settings.rs:80` | **Missing** | `palantir-desktop` |
| 10 | **Feature-flag enum** gating unfinished surfaces | `SOURCE settings.rs:96-107` | **Missing** | `palantir-core` |

> **Settings is the whole top ten.** That is the owner's complaint, confirmed
> from both directions: they have 46 persisted settings across 10 sections; we
> have 7 tabs and no schema-owned defaults, no hooks, no args, no env vars, no
> directory setting. Nothing else in this analysis comes close in weight.

---

## 2 — Settings surface

| Modrinth capability | Evidence | Status | Our target |
| --- | --- | --- | --- |
| Appearance: theme (5 values incl. retired `retro`) | `SOURCE settings.rs:434` | **Partial** — we have `Accent` (`model.rs:268`) + `Scale` (`model.rs:291`) | `palantir-theme` |
| Appearance: OLED / Sync-with-system themes | `OBSERVED S-0008` | **Missing** | `palantir-theme` |
| Appearance: "sync across devices" | `SOURCE settings.rs:22-25` | **Out-of-scope** — we have no account | — |
| Appearance: advanced rendering (+ "requires restart" warning) | `OBSERVED S-0020+` | **Missing** | `palantir-desktop` |
| Appearance: native decorations | `SOURCE settings.rs:20` | **Missing** — iced owns this | `palantir-desktop` |
| Appearance: compact instance cards | `SOURCE settings.rs:67` | **Missing** | `palantir-desktop` |
| Appearance: show play time | `SOURCE settings.rs:69` | **Missing** | `palantir-core` |
| **Features: toggle instance tabs** | `SOURCE settings.rs:26-33` | **Missing** | `palantir-desktop` |
| Behavior: launch page (`home`/`library`) | `SOURCE settings.rs:16` | **Partial** — we have `Page`, no preference | `palantir-desktop` |
| Behavior: hide on process start | `SOURCE settings.rs:63` | **Missing** | `palantir-desktop` |
| Behavior: refocus on game close | `SOURCE settings.rs:65` | **Missing** | `palantir-desktop` |
| Behavior: warn on unknown modpacks | `SOURCE settings.rs:71` | **Missing** | `palantir-loader` |
| Behavior: max concurrent downloads / writes | `SOURCE settings.rs:11-12` | **Missing** | `palantir-net` scheduler |
| Language: 33 locales, Crowdin, marked **Beta** | `OBSERVED S-0037+` | **Partial** — we have `locale.rs` | `palantir-desktop` |
| **Java installations** (discovery, add, remove) | `SOURCE state/java_globals.rs` | **Partial** | `palantir-loader` |
| Memory max heap, system-derived default | `SOURCE settings.rs:425-432` | **Partial** | `palantir-loader` |
| Game resolution (854×480 default), force fullscreen | `SOURCE settings.rs:62,481` | **Partial** | `palantir-loader` |
| **Extra launch args, custom env vars** | `SOURCE settings.rs:58-59` | **Missing** | `palantir-loader` |
| **Hooks: pre-launch, wrapper, post-exit** | `SOURCE settings.rs:74,487` | **Missing** | `palantir-loader` |
| **Custom data directory** (+ recoverable move) | `SOURCE settings.rs:76-77` | **Missing** | `palantir-core` |
| Developer mode | `SOURCE settings.rs:80` | **Missing** | `palantir-desktop` |
| App auto-update tri-state, skip/defer | `SOURCE settings.rs:83-85` | **Missing** | `palantir-net` |
| Profile / Social / Privacy sections | `OBSERVED S-0008` | **Partial** — we have `Privacy` | `palantir-desktop` |
| Friends presence | `SOURCE state/friends.rs` | **Out-of-scope** | — |
| Telemetry, personalized ads, Discord RPC | `SOURCE settings.rs:54-56` | **Out-of-scope** | — |
| Skin selector | `OBSERVED S-0004` | **Out-of-scope** | — |
| Servers / hosting surfaces | `OBSERVED S-0006`; feature flags `ServersInApp`, `ServerProjectQa` | **Out-of-scope** | — |

## 3 — Navigation & shell

| Modrinth capability | Evidence | Status | Our target |
| --- | --- | --- | --- |
| Icon-only left rail, 5 fixed destinations | `OBSERVED S-0001` | **Partial** — we have `Page`, sidebar present | `palantir-desktop` |
| Breadcrumb / route title in the titlebar | `OBSERVED S-0002,S-0003` | **Missing** | `palantir-desktop` |
| Settings as a **modal** over the instance list | `OBSERVED S-0008` | **Missing** — ours is a page | `palantir-desktop` |
| Window min-size contract (1100×700) | `SOURCE tauri.conf.json` | **Equivalent, deliberately not copied** — ours goes to 840×600 (`UI-HANDOFF.md`) | — |
| Custom titlebar, `decorations: false` | `SOURCE tauri.conf.json` | **Missing** | `palantir-desktop` |
| Account avatar menu in the titlebar | `OBSERVED S-0001` | **Partial** | `palantir-desktop` |
| Running-instance status pill | `OBSERVED S-0001` | **Partial** — we have `Panel::Launch` | `palantir-desktop` |
| Connectivity indicator | `OBSERVED S-0001` | **Missing — and see the warning below** | `palantir-net` |

> **Do not copy the connectivity chip.** Modrinth's is `!navigator.onLine`
> (`App.vue:465`) and reports **Offline while the network is fully working**
> (`OBSERVED S-0003`, `open-questions.md` OQ-004). It gates controls on a
> browser-level guess while a second layer does the real requests. If we add an
> online indicator it must come from the request layer, never from a UI-level
> guess.

## 4 — Discover / browse

| Modrinth capability | Evidence | Status | Our target |
| --- | --- | --- | --- |
| Six project-type tabs | `OBSERVED S-0003` | **Partial** — `ProjectType` exists (`model.rs:711`) | `palantir-desktop` |
| Search with debounce | `OBSERVED S-0003` | **Equivalent** — 350 ms debounce (`UI-HANDOFF.md`) | `palantir-net` |
| Sort select | `OBSERVED S-0003`; `Sort` (`model.rs:743`) | **Partial** | `palantir-net` |
| View-density select (20 per page) | `OBSERVED S-0003` | **Missing** | `palantir-desktop` |
| Pagination | `OBSERVED S-0003` | **Missing** — ours scrolls | `palantir-net` |
| Project card: downloads, follows, badges, loader, date | `OBSERVED S-0003` | **Partial** — `design.rs:266` `project_card` | `palantir-desktop` |
| Facet sidebar, "Hide already installed" | `OBSERVED S-0003` | **Missing** | `palantir-net` |
| Project detail tabs (Description/Versions/Gallery) | `SOURCE routes.js:77-105` | **Missing** | `palantir-desktop` |
| Install flow with **optional dependency** choice | `SOURCE` app flow; `UI-HANDOFF.md` notes `.mrpack` optional files already carried | **Partial** — review dialog exists | `palantir-loader` |

## 5 — Library / instance management

| Modrinth capability | Evidence | Status | Our target |
| --- | --- | --- | --- |
| Instance list + content tabs | `SOURCE routes.js:106-150` | **Partial** — `{Overview, Content, Worlds, Logs, Manage}` | `palantir-desktop` |
| Context menus on an instance | `SOURCE` library components | **Missing** | `palantir-desktop` |
| Pin / duplicate / delete-with-confirm | `SOURCE` library components | **Partial** — `Panel::RemoveAccount` pattern exists | `palantir-desktop` |
| Import from other launchers | `OBSERVED S-0001` ("Import from launcher") | **Partial** | `palantir-loader` |
| Add-instance dialog (vanilla / loader / modpack) | `OBSERVED S-0007` | **Partial** | `palantir-desktop` |
| Worlds: list, play, edit, delete, export, open folder | `SOURCE routes.js:111-116` | **Partial** — world export already real (`UI-HANDOFF.md`) | `palantir-desktop` |
| Logs view | `SOURCE routes.js:142-148` | **Partial** — `InstanceTab::Logs` | `palantir-desktop` |
| Files browser | `SOURCE routes.js:136` | **Missing** | `palantir-desktop` |
| Screenshot gallery | `OBSERVED S-0005` | **Out-of-scope** | — |
| Share / publish instance | `SOURCE routes.js:116` | **Missing** | `palantir-desktop` |

## 6 — Launch & runtime

| Modrinth capability | Evidence | Status | Our target |
| --- | --- | --- | --- |
| Launch state machine with per-state affordances | `SOURCE` launcher | **Partial** — `LaunchMode` (`model.rs:68`), `Panel::Launch` | `palantir-desktop` |
| Concurrent launches / queue | `SOURCE` | **Missing** | `palantir-desktop` |
| Crash reports surfaced with next steps | `SOURCE` | **Partial** | `palantir-desktop` |
| Java-missing / wrong-version diagnosis | `SOURCE` | **Partial** | `palantir-loader` |
| Post-exit behaviour | `SOURCE` | **Partial** | `palantir-desktop` |
| Live log tail | `SOURCE routes.js:146` | **Partial** | `palantir-desktop` |

## 7 — Accounts

| Modrinth capability | Evidence | Status | Our target |
| --- | --- | --- | --- |
| Microsoft device-code sign-in | `SOURCE` `apps/app/src/api/auth.rs:29`; `modules/07-accounts-auth.md` AUTH-6 | **Partial** — a Microsoft session is stored and injected (`UI-HANDOFF.md`) | `palantir-net` |
| Modrinth website sign-in (sealed token, no client secret) | `SOURCE` `apps/app/src/api/mr_auth.rs:29`; AUTH-2 | **Missing** | `palantir-net` |
| Multi-account switcher | `SOURCE` `AccountsCard.vue`; AUTH-7 | **Missing** | `palantir-desktop` |
| Token refresh + expiry handling | `SOURCE` AUTH-8 | **Partial** | `palantir-net` |
| Gated surfaces that ask rather than fail | `SOURCE` AUTH-10 | **Partial** | `palantir-desktop` |
| Offline identity fallback | `SOURCE` `Authentication::Offline` (`model.rs:74` both sides) | **Equivalent** | — |

## 8 — Cross-cutting

| Modrinth capability | Evidence | Status | Our target |
| --- | --- | --- | --- |
| Onboarding checklist following the user | `OBSERVED S-0001`; `onboarding_checklist` table | **Partial** | `palantir-desktop` |
| Keyboard shortcuts advertised in-UI | `OBSERVED S-0001` (`N` to quick-create) | **Partial** — F1 help exists | `palantir-desktop` |
| 33 locales, Crowdin | `OBSERVED`; `locales/` | **Partial** — `locale.rs` | `palantir-desktop` |
| Review dialog with in-dialog receipt | `UI-HANDOFF.md` | **Equivalent** — ours already does this | — |
| Empty/loading/error state patterns | `OBSERVED S-0001` | **Partial** | `palantir-desktop` |
| Destructive-action confirmation | `SOURCE` | **Equivalent** — reviews exist | — |

---

## What we already do better — do not regress these

A gap analysis that only counts misses is a to-do list. Three of our behaviours
are ahead of the reference and should survive contact with it:

1. **We have a narrow layout; they do not.** Modrinth's floor is 1100×700
   (`SOURCE tauri.conf.json`) — there is no small-window layout to copy. Our
   smoke test exercises 840×600 (`UI-HANDOFF.md`), which is a real capability
   they do not have.
2. **In-dialog receipts.** A confirmed operation's outcome renders inside the
   open review dialog rather than in a banner hidden behind the modal scrim
   (`UI-HANDOFF.md`). Modrinth's reviewed flows still lean on toasts
   (`OBSERVED S-0012` shows a toast layered over a dialog).
3. **Byte-exact local operations.** World export and local pack import are
   verified against actual bytes, and overwrites are refused before writing. The
   mission could not verify Modrinth's equivalents at all, because they require
   an account and a populated library.

Two things we must **stop copying** if we ever do:

4. **Their connectivity indicator is wrong** (`App.vue:465`, `OBSERVED S-0003`).
5. **Their theme offers a value the UI no longer exposes** (`retro`, still in
   `enum Theme`, absent from the picker) — a migration lesson *for* us, not a
   pattern to copy blindly.