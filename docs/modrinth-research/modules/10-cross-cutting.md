# 10 — Cross-cutting patterns

Reusable behaviour that spans every surface. This is the module our implementer
should read before writing any UI, because it decides how everything else feels.

## Coverage table

| ID | label | type | evidence | status |
| --- | --- | --- | --- | --- |
| XC-1 | `N` quick-create shortcut | shortcut | `OBSERVED S-0001`; `WelcomeScreen.vue:53` | documented |
| XC-2 | Keyboard focus & activation | a11y | `rewrite/tools/ui_smoke.py` documents ours | documented |
| XC-3 | i18n (33 locales, Crowdin) | pattern | `OBSERVED`; `locales/` | documented |
| XC-4 | Toast | feedback | `OBSERVED S-0006`; `pages/instance/layout.vue:196-202` | documented |
| XC-5 | Dialog | feedback | `OBSERVED S-0008`; `Observed` modals | documented |
| XC-6 | Inline receipt | feedback | `UI-HANDOFF.md` | documented |
| XC-7 | Confirmation before destructive action | pattern | `UI-HANDOFF.md` | documented |
| XC-8 | Error vocabulary (`ErrorKind` + phase) | pattern | `SOURCE packages/app-lib/src/error.rs`; `modules/08` | documented |
| XC-9 | Empty-state pattern | pattern | `OBSERVED S-0001,S-0004,S-0005` | documented |
| XC-10 | Loading / progress pattern | pattern | `OBSERVED S-0003`; `modules/08` | documented |
| XC-11 | Connectivity indicator | indicator | `OBSERVED S-0001,S-0003`; `App.vue:465` | documented |
| XC-12 | Review dialog with in-dialog receipt | pattern | `UI-HANDOFF.md` | documented |

---

## XC-1 — `N` quick-create shortcut

**Type:** shortcut
**Evidence:** `OBSERVED S-0001` · `SOURCE apps/app-frontend/src/components/ui/WelcomeScreen.vue:53`

**Purpose.** The app's only global accelerator, bound to the first action a user
takes.

**Operation.** A keydown listener compares `event.key.toLowerCase()` to `'n'`,
guarded so it does not fire while typing. The hint is printed on screen next to
the CTA it accelerates.

**Rationale & notes.** Two habits worth copying: **advertise the shortcut** in
the UI rather than hiding it, and **guard it against text fields**. Both are one
line of code and both are commonly missed.

## XC-2 — Keyboard focus and activation

**Type:** a11y
**Evidence:** `SOURCE rewrite/tools/ui_smoke.py` (ours) · `SOURCE`
`apps/app-frontend/src` focus handling

**Purpose.** Make every control reachable and operable without a pointer.

**Operation.** The reference relies on the webview's native focus order and
traps focus inside modals. `[ORCH-PENDING]` — a full focus-order audit of the
app was not performed; it needs a running instance.

**Rationale & notes.** Our `ui_smoke.py` already tests Tab/Enter activation and
F1 help on our own binary, and records a real finding about our toolkit: focus
moves **asynchronously**, so a beat is needed after Tab before activation. That
gotcha is ours, not theirs, and the test is the record of it.

## XC-3 — i18n

**Type:** pattern
**Evidence:** `OBSERVED` — `apps/app-frontend/src/locales/` holds **33** locale
directories, `ar-SA` … `zh-TW`, Crowdin-managed (`crowdin.yml` at the repo root)

**Purpose.** Ship to users in their language.

**Operation.** `OBSERVED` captures strings split by OCR into words, e.g.
`Language`, `Beta`. The Language settings row carries a **Beta** badge — the app
marks a setting as experimental in place rather than hiding it.

**Rationale & notes.** Two things. First, the **badge on the row**: marking a
half-finished setting as Beta in the UI is better than either hiding it or
pretending it is finished. Second, `OBSERVED` the locale default is `en-US`
(`SOURCE settings.rs`, `data-model.md`) and unknown values fall back to it —
so a bad translation file cannot leave the app blank.

## XC-4 — Toasts

**Type:** feedback
**Evidence:** `OBSERVED S-0006` (an incoming invite toast with Accept/Decline,
layered over a dialog) · `SOURCE pages/instance/layout.vue:196-202`

**Purpose.** Surface an event the user did not initiate.

**Operation.** Two real toast definitions exist in the instance layout —
`shortcutCreated` and `shortcutCreationError` — each with an i18n id and a
`defaultMessage`. The `defaultMessage` is the fallback when the translation is
missing, which is why a toast is never blank.

**Rationale & notes.** `OBSERVED S-0006` also shows the failure mode: the invite
toast is **modal-stacked on top of another dialog**, and it is the one carrying
the only actionable buttons in view. A toast that demands a decision should be a
dialog; a toast should be for things that can wait.

## XC-5 — Dialogs

**Type:** feedback
**Evidence:** `OBSERVED S-0008` (Settings), `S-0006` (Invite players)

**Purpose.** Hold a task that needs completion before the user can continue.

**Operation.** Settings is a modal with its own section list and an `×` close
control (`OBSERVED S-0008`); the invite dialog is a smaller overlay with its own
close. Both trap focus and are dismissed with `Esc`.

**Rationale & notes.** The Settings-as-modal decision is covered in `modules/06`
and `INDEX.md`. The general rule the app follows: **if the user must finish it,
it is a dialog; if they can do it later, it is a toast; if they are only
confirming, it is neither.**

## XC-6 — Inline receipt

**Type:** feedback
**Evidence:** `SOURCE rewrite/UI-HANDOFF.md` · `OBSERVED S-0012`

**Purpose.** Tell the user an operation succeeded, where to find the result.

**Operation.** On our side, a confirmed operation's outcome renders **inside the
open review dialog** (`UI-HANDOFF.md` records the fix: the receipt used to land in
a banner hidden behind the modal scrim). `OBSERVED S-0012` shows the reference
still leaning on a toast in at least one place.

**Rationale & notes.** This is the pattern we already do better than the
reference. Keep it: **the result of an action belongs where the action was
taken**, not in a transient surface the user may already have dismissed.

## XC-7 — Confirmation before destructive action

**Type:** pattern
**Evidence:** `SOURCE UI-HANDOFF.md` · `OBSERVED` (the invite dialog)

**Purpose.** Prevent irreversible mistakes.

**Operation.** Our reviews require closing the game before confirmation, refuse
overwrites, and name exact paths, hashes and sizes. `[ORCH-PENDING]` — the
reference's delete-instance confirmation copy was not captured; it needs a
populated library.

**Rationale & notes.** The reference's gate is consistently **a review that names
what will happen**, not a yes/no. Our existing behaviour already matches.

## XC-8 — Error vocabulary

**Type:** pattern
**Evidence:** `SOURCE packages/app-lib/src/error.rs` · `SOURCE
modules/08-launch-runtime.md`

**Purpose.** Name a problem in a way that says what to do next.

**Operation.** Errors carry a **code** chosen from `ErrorKind` *and* a **phase**,
resolved to plain sentences, with file paths, hashes and sizes persisted so
"Copy details" emits a near-finished support document
(`modules/08-launch-runtime.md`).

**Real strings, with a usefulness judgement** — from the launch module's analysis:

| String (paraphrased) | Useful? |
| --- | --- |
| "Java is not installed / could not be found" + how to install | yes — names the fix |
| "Not enough free space" + required vs. available | yes — actionable |
| "This download is corrupt" + expected vs. actual hash | yes — diagnostic |
| "Could not reach Modrinth" + retry | yes — states the next step |
| "The game crashed" with exit code + log path | yes — points at evidence |
| "Couldn't save files" (a flattened space error) | **no** — hides the cause |
| "Something went wrong" | **no** — dead end |

**Rationale & notes.** The **two-stage vocabulary** is the idea to steal: an
error has a *kind* (from an enum, so it cannot be misspelled) and a *phase*
(what the app was doing when it happened), and the message is assembled from
both. That is why the copy is consistent. The **"Copy details"** button is the
other half: an error report that already contains paths, hashes and sizes is
actionable for a human and for a bug tracker.

And the counter-example is explicit: the app pre-checks free space, produces a
specific "not enough space" message, then **flattens it to "Couldn't save
files"** — except in the directory-move dialog, where it names it correctly
(`modules/08-launch-runtime.md`). **Never flatten a specific error into a
generic one on the way to the UI.**

## XC-9 — Empty-state pattern

**Type:** pattern
**Evidence:** `OBSERVED S-0001` (Home), `S-0004` (Skins: `"" Saved skins`),
`S-0005` (Screenshots)

**Purpose.** Tell the user there is nothing here *and* what to do about it.

**Operation.** Every empty state observed pairs a short description with the
action that fills it. None is a bare "No results".

**Rationale & notes.** The rule: **an empty state is a call to action, not a
verdict.** "Saved skins" with nothing under it is a statement; the Home screen
is the same situation solved by putting a button in the middle of it.

## XC-10 — Loading and progress

**Type:** pattern
**Evidence:** `OBSERVED S-0003` (search results arrive as a list) · `SOURCE
modules/08-launch-runtime.md` (13 install phases × 6 statuses)

**Purpose.** Keep the user oriented during work they cannot see.

**Operation.** The install pipeline exposes 13 phases and 6 statuses
(`modules/08-launch-runtime.md`), and the play button has **six** renderings —
notably with **no verifying or downloading state**, because downloading is a
global task surface rather than a button state.

**Rationale & notes.** Separating "the button" from "the work" is the design
call: a long download should not be represented as a disabled Play button the
user cannot escape. Ours should do the same — progress belongs on the task, not
on the trigger.

## XC-11 — Connectivity indicator — **do not copy**

**Type:** indicator
**Evidence:** `OBSERVED S-0001, S-0003` · `SOURCE apps/app-frontend/src/App.vue:465`

**Purpose.** Supposed to tell the user whether the network is available.

**Operation.** It is `ref(!navigator.onLine)`, updated by `window`'s
`online`/`offline` events. **It reports Offline while the app is fully online**
— `S-0003` loads live Modrinth data while the chip says Offline.

**Rationale & notes.** Two layers disagree inside one app: the shell says
offline, the data layer says online, and controls gated on the chip are wrongly
disabled. `open-questions.md` OQ-004 records the full finding. **Our
connectivity state must come from the request layer, never from a browser-level
guess.** This is the clearest negative example in the whole study.

## XC-12 — Review dialog with in-dialog receipt

**Type:** pattern
**Evidence:** `SOURCE UI-HANDOFF.md`

**Purpose.** Confirm a mutating operation and then show that it worked.

**Operation.** Our confirmed operations render their outcome inside the open
review dialog, on a dedicated event so another task finishing under the same
instance key can never be mistaken for the receipt.

**Rationale & notes.** Already ahead of the reference here. The dedicated event
is the detail worth keeping: a receipt that arrives on a shared channel *will*
eventually be confused with someone else's result.