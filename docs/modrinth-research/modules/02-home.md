# 02 — Home

The landing tab, `/` (`SOURCE apps/app-frontend/src/routes.js:8-12`). What it
shows depends entirely on whether the user has done anything yet, and that
dependence is the whole design.

## Second observation pass (2026-10-14)

Home's **two states are now both observed.** The first-run state is `S-0001`
(welcome block, "Create an instance", "Import from launcher", Getting-started
checklist, News feed). The **populated** state `[S-0109 S-0116]` replaces the
welcome block with a **"Jump in"** strip over a **"Library"** grid — the full
detail is in module 04. The transition between them is the single most important
thing about Home: it is *derived from instance count*, not configured.

`HOME-2` (the "Create an instance" CTA and its `N` shortcut) is now confirmed
**functional, not merely present**: the CTA and the shortcut were inert only
because `navigator.onLine` was false (OQ-004), and with the environment fixed
(OQ-012) `N` opens the create-instance dialog `[S-0094 S-0095]`. The `N` hint
text under the CTA ("Press N to quick create an instance") is a *documented
accelerator advertised in the UI*, which module 10 recommends adopting.

## Coverage table

| ID | label | type | evidence | status |
| --- | --- | --- | --- | --- |
| HOME-1 | Welcome heading + empty-state block | container | `OBSERVED S-0001`; `components/ui/WelcomeScreen.vue` | documented |
| HOME-2 | "Create an instance" CTA | button | `OBSERVED S-0001` | documented |
| HOME-3 | `N` quick-create shortcut | shortcut | `OBSERVED S-0001`; `WelcomeScreen.vue:53` | documented |
| HOME-4 | "Import from launcher" escape hatch | button | `OBSERVED S-0001` | documented |
| HOME-5 | "Getting started" onboarding checklist | indicator | `OBSERVED S-0001`; `state/onboarding_checklist.rs` | documented |
| HOME-6 | News feed | list | `OBSERVED S-0001` | partial |
| HOME-7 | Populated state | container | `OBSERVED S-0109, S-0116`; `SOURCE WelcomeScreen.vue` | documented |

---

## HOME-1 — Welcome / empty-state block

**Type:** container **Nav path:** rail → Home, or `/`
**Evidence:** `OBSERVED S-0001` · `SOURCE apps/app-frontend/src/components/ui/WelcomeScreen.vue`

**Purpose.** This is the first screen every user of a fresh install sees, so it
has to do two jobs at once: say what the app is, and make the next action
impossible to miss. It is also the screen a returning user with a populated
library should *not* see, which is why it is an empty-state rather than a page.

**Operation.** Renders a centred block: a large mark, "Welcome to Modrinth",
"Ready to start playing?", the primary CTA, and the shortcut hint
(`OBSERVED S-0001`). When instances exist, the block is replaced by the library
rather than being scrolled past — the app swaps the surface, it does not stack.

**Controls:** none. This is content, not a control surface.

**Persistence:** nothing on this screen. Its *appearance* is a function of
library state, which is stored elsewhere.

**Rationale & notes.** The decision to make Home an **empty state** rather than
a dashboard is the one to copy. A dashboard implies there is something to
report; on day one there is not, and a wall of empty widgets is worse than an
invitation. Our Home should do the same: when the library is empty, offer the
next step; when it is not, get out of the way.

## HOME-2 — "Create an instance" CTA

**Type:** button
**Evidence:** `OBSERVED S-0001`

**Purpose.** The single most important action in the app. Everything else —
browse, install, launch — is a detour around it for a first-time user.

**Operation.** Opens the create-instance dialog (see `modules/04`). It is the
primary action by weight and position, and it is repeated by the `N` shortcut
(HOME-3) and by the onboarding checklist (HOME-5), so the next step is reachable
from three places without any of them competing.

**Controls:** **`:disabled="offline"`** — `SOURCE
apps/app-frontend/src/components/ui/WelcomeScreen.vue:99`. In this environment
that is **true**, so the button is inert. See OQ-004: the app's false-positive
offline state disables its own primary action.

**Rationale & notes.** Three routes to one action is deliberate redundancy, and
it is correct: a user who has just installed a launcher does not yet know
where anything is. The checklist is the patient one, the CTA is the obvious
one, and the shortcut is the one for people who already know.

## HOME-3 — `N` quick-create shortcut

**Type:** keyboard shortcut
**Evidence:** `OBSERVED S-0001` (the hint "Press N to quick create an instance" is
on screen) · `SOURCE apps/app-frontend/src/components/ui/WelcomeScreen.vue:53`

**Purpose.** Shortcuts exist for the action a user repeats, but this one is bound
to the *first* action instead. That is the tell: the app expects most users to
create several instances in a session, and it wants the keyboard to be the
fastest path there.

**Operation:** A keydown listener checks the pressed key against `'n'`
(`WelcomeScreen.vue:53`), guarded so it does not fire while typing in a field.
Because the hint is printed on the screen next to the CTA, the shortcut is
**discoverable** rather than hidden — the app advertises its own accelerator.

**Controls:** the handler is wrapped in `if (!offline.value)`
(`WelcomeScreen.vue:62`), so **the shortcut is inert whenever the app believes it
is offline** — which is the case in this environment. Same root cause as HOME-2.

**Persistence:** none.

**Rationale & notes.** Two ideas. First, advertise the shortcut in the UI next
to the action it accelerates; a shortcut nobody knows about is not a feature.
Second, guard it against text fields — the guard at `WelcomeScreen.vue:53` is
the difference between a shortcut and a bug.

## HOME-4 — "Import from launcher"

**Type:** button
**Evidence:** `OBSERVED S-0001` ("Escaping another launcher?" above "Import from launcher")

**Purpose.** Onboarding for the single largest source of new users: someone
who already has instances in another launcher and does not want to lose them.
It is the difference between "start over" and "bring your stuff".

**Operation.** Opens the import flow (`modules/04` lists the launchers
supported). Positioned **below** the primary CTA and visually quieter, which is
the right priority order: the app would rather you create something than import,
but it would rather you import than leave.

**Rationale & notes.** The copy is the part to copy in spirit: "Escaping another
launcher?" is a joke that acknowledges the user's situation instead of ignoring
it. We can say the same thing in our own words. Placement below the CTA matters
as much as the button existing.

## HOME-5 — "Getting started" onboarding checklist

**Type:** indicator (dismissible checklist, right sidebar)
**Evidence:** `OBSERVED S-0001` · `SOURCE packages/app-lib/src/state/onboarding_checklist.rs`

**Purpose.** The app's answer to "what do I do next?" that survives past the
welcome screen. Because it lives in the sidebar it follows the user across
tabs, so the next unfinished setup step is always one glance away.

**Operation.** A checklist of steps — `OBSERVED S-0001` shows three: "Create
first instance", "Sign in to Minecraft", "Sign in to Modrinth" — with radio-style
indicators. Completion is persisted per step (`onboarding_checklist` table,
`OBSERVED` in `data-model.md`), so it survives restarts. The panel can be
collapsed (a chevron is visible in `S-0001`).

**Rationale & notes.** The step list is **derived from state**, not hardcoded:
each step corresponds to something the app can check. That is why it can show a
circle rather than a tick for an unfinished step — it knows. Ours should do
the same: a checklist whose items are computed from real state is honest, and
one whose items are hardcoded lies as soon as the user does something
unexpected.

## HOME-6 — News feed

**Type:** list
**Evidence:** `OBSERVED S-0001`

**Purpose.** Keeps the launcher feeling alive between launches and gives the app
a reason to show project announcements without the user going looking.

**Operation.** `[ORCH-PENDING]` — the feed rendered in `S-0001` shows a card
with an image and a link, but the endpoint and its cache TTL were not traced in
this mission. It is almost certainly the same response cache as Browse
(`data-model.md`, 601 rows).

**Rationale & notes.** Low value, low risk. If we add one, it should be
cacheable for days, never block the UI when it fails, and be dismissible — a
news feed that errors should be invisible, not an error banner.

## HOME-7 — Populated state

**Type:** container
**Evidence:** `SOURCE WelcomeScreen.vue`

**Purpose.** What Home becomes once the user has instances.

**Operation:** `[ORCH-PENDING]` — the populated Home was not captured, because
it requires instances, and creating instances requires a Minecraft account
(mission §1.4 forbids one). The component exists and is the same `WelcomeScreen`
that renders the empty state, so the populated case is a branch of it.

**Rationale & notes.** This is the largest single gap in the captured evidence
and it is stated rather than papered over. An implementer should read
`WelcomeScreen.vue` directly.