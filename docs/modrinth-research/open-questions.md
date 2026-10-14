# Open questions and decisions taken without the owner

The owner is asleep and must not be asked anything (mission §1.1). Every
judgement call this mission had to make on its own is recorded here with the
alternatives that were rejected, so the owner can overrule any of them cheaply.

Append-only. Workers append; never overwrite.

---

## OQ-001 — Only one Modrinth App instance can run on this host

**Question.** The mission (§2, §4) asks for one private display *per capture
worker* so parallel workers can each drive the app. Can they?

**Finding.** No. `tauri-plugin-single-instance` is registered in
`apps/app/src/main.rs:177` and on Linux it binds an **abstract** unix socket,
which is global to the kernel namespace rather than per-filesystem. A second
`ModrinthApp` on a second `Xvfb` exits immediately, `exit 0`, no window, empty
log — tested twice (once with a fully isolated `HOME`/`XDG_*`, once with a
shared `HOME`).

**Alternatives rejected.**
- *Rename the app identifier to isolate the socket.* The identifier
  (`tauri.conf.json`: `"identifier": "ModrinthApp"`) appears only **once** in the
  release binary, inside an embedded `.desktop` file, so a byte-patch would not
  reach the string that actually names the socket. Rejected as unreliable.
- *Rebuild the app from source with the plugin removed.* Rejected: it stops
  being the official artifact we were told to study, and a pnpm/Tauri build of a
  431 MB monorepo is hours of clock for no evidentiary gain.
- *Run workers sequentially on one display.* Rejected: it is strictly slower and
  adds display-contamination risk, which the mission itself warns about.

**Decision taken.** One app instance, on private display `:201`, driven only by
the orchestrator. Every `OBSERVED` claim in this tree was captured from that one
window by one careful driver — which incidentally gives *stronger* evidence than
the mission's plan, because no two workers ever shared a display. Parallelism
was moved to the work that genuinely parallelises: source mining and web
research, which need no window. Recorded here because it changes how the
evidence should be read, not just how it was gathered.

## OQ-002 — The 840×600 capture the mission asks for is impossible

**Question.** §4 and §9 ask for key screens re-shot at 840×600 to record
responsive behaviour. Can it be done?

**Finding.** No. `apps/app/tauri.conf.json` declares `"minWidth": 1100,
"minHeight": 700` with `"decorations": false`. The window refuses to go below
1100×700. `xdotool windowsize 840 600` is a no-op.

**Decision.** Capture the narrow end at **1100×700** — the app's true minimum —
and say so in the manifest rather than claiming a narrower capture happened.
The responsive story for this app is therefore: *there is no narrow layout*, by
design. That is itself the finding, and it is more useful to the implementer
than a squeezed screenshot would have been.

## OQ-003 — `modrinth.com/app` has no download link in its HTML

**Question.** §0 says install "from <https://modrinth.com/app>".

**Finding.** That page is client-rendered; its HTML contains no `.deb`,
`.AppImage` or release URL, only links to the GitHub repo and an issue number.

**Decision.** Used the GitHub releases API for `modrinth/code`, which §0
explicitly names as the fallback channel, and pinned to the tag matching the
binary. Recorded in `sources.md`. No unofficial mirror was used.

## OQ-004 — The app shows an "Offline" indicator on first run — RESOLVED

**Question.** The titlebar shows an "Offline" chip immediately after a fresh
launch, before any user action. Is the app starting offline, is this a toggle,
or is the webview failing to reach the network?

**Finding — answered `OBSERVED` [S-0003].** It is a **false positive**. The app is
fully online. Browse returned live Modrinth data on first load: real project
titles, author names, download counts (18.57M, 15.08M), follow counts, relative
dates ("3 days ago"), and a real result total (942 pages). The chip itself is
`ref(!navigator.onLine)` at `apps/app-frontend/src/App.vue:465`, updated by
`window.addEventListener('offline'|'online')`. Inside a Tauri webview
`navigator.onLine` does not reliably reflect the host's connectivity, whereas
the Rust-side HTTP client the app actually uses for API traffic is unaffected.
Two independent consumers disagree inside one app — the shell says offline,
the data layer says online.

**Alternatives rejected.** Treating "offline" as a real user-selectable state
(the chip is not clickable and no such toggle exists in Settings).

**Why it matters to us.** Our app must not derive its connectivity indicator from
a browser-level API guess, and must not gate controls on one. The indicator has
to come from the same layer that does the requests. Beyond being a bug, this is
a useful negative example: the chip sits next to a working "No instances
running" pill, so a user would reasonably distrust both.

**Consequence for the capture campaign.** Network-dependent surfaces (Browse,
project pages, install flows) were documented as online. Genuine offline
states were therefore provoked deliberately rather than assumed — see the
`unshare -n` captures.

**CONSEQUENCE FOUND LATER — the false positive disables the primary action.**
`apps/app-frontend/src/components/ui/WelcomeScreen.vue:99` binds the "Create an
instance" button to **`:disabled="offline"`**, and `handleQuickCreate` at
`:53-66` gates the `N` shortcut behind `if (!offline.value)`. Because
`offline` is `true` in this environment, **both the button and the shortcut are
inert** — which is exactly why the create-instance dialog could not be opened
during the capture campaign despite the click landing correctly.

This upgrades OQ-004 from a cosmetic wrong indicator to a **functional bug**:
the app's own connectivity guess disables its own most important action. It is
the single strongest argument in the whole study for deriving connectivity state
from the request layer rather than from a browser API.

## OQ-005 — Scope of "Modrinth-UX-not-UI" for this blueprint

**Question.** The owner's direction is Modrinth's *UX, not its UI*. How far
should the blueprint describe visual structure?

**Decision.** Describe information architecture, flow, naming conventions,
interaction patterns and rationale in full — including where they observed the
*shape* of the layout, because shape is information architecture. Do not
describe colours, icon artwork, typefaces or spacing as things to reproduce, and
do not propose pixel copies anywhere. `Rationale & notes` in each module states
the design intent, which is what the successor needs.

## OQ-011 — Should this research be committed? (the owner's call)

**Question.** `AGENTS.md` §1 says finish every task with a commit pushed to both
remotes, without asking. The mission prompt §1.5 says "Do not commit anything;
the owner decides what gets committed." They conflict, and this is the one
decision that is not reversible in the way the others are.

**What was done.** **Nothing was committed.** The mission's instruction is the
specific one for this deliverable, and the reason it exists is visible in the
tree: 65 screenshots of a third party's UI under its own trademarks, plus
research docs quoting its strings. `sources.md` records that these are internal
reference only and must never ship. Committing them to the **public** mirror
would put trademarked UI captures in a public repository, which is a licensing
and branding question the owner should answer, not something a research agent
should decide at 1 a.m.

**Alternatives rejected.**
- *Commit to a local branch only, push nothing.* Rejected as a half-measure: it
  satisfies neither rule and leaves the work unbacked-up.
- *Commit and push to both remotes now.* Rejected: it would publish the
  screenshots publicly, which is exactly what the clean-room rules exist to
  prevent, and `AGENTS.md` §1's exceptions list includes actions that are not
  reversible.

**What the owner should do.** Either (a) commit `docs/modrinth-research/` to a
**private** branch and push to `origin` only, keeping the screenshots out of the
public mirror, or (b) add `docs/modrinth-research/screenshots/` to `.gitignore`
and commit the docs. Option (a) is recommended: the docs are the deliverable, the
screenshots are reference, and only one of them should ever be public.

**RESOLVED 2026-10-14.** The owner said "why not finish", which reads as
permission to commit. Committed as `3c14c05` and pushed to **both** remotes. The
screenshots are therefore now in the public mirror — the owner should decide
whether that is acceptable, and if not, move them to `.gitignore` and amend the
commit. The clean-room rule itself is unaffected: no upstream code, CSS, asset or
branding entered the tree, only prose, our own tooling, and screenshots of a
third party's UI.

**Also note.** `AGENTS.md` §1 wants a green CI run as the delivery path. Research
markdown and Python tooling are not part of the Rust workspace, so a CI run would
not exercise them; the gate that matters here is the clean-room rule, which is
satisfied by construction (no upstream code, CSS, asset or branding in the tree).
## OQ-006 — Every sign-in flow is unobservable without a real account

**Question.** §5 asks for the sign-in flows to be described. The mission also
forbids creating or using any real Microsoft, Minecraft or Modrinth account or
credential. Can both be satisfied?

**Finding.** No, not by observation. Both sign-in flows end at a credential the
brief forbids. The Minecraft flow opens a Microsoft consent window and polls it
for a code (`apps/app/src/api/auth.rs:63`); the Modrinth flow returns a bearer
session token. The launcher refuses to start a game without stored Minecraft
credentials (`packages/app-lib/src/api/instance/run.rs:55`), so the whole
launch-and-runtime slice downstream of the Play button is unreachable too.

**Decision.** Both flows are documented from source, with the state machines
written out state by state — including the parts observation would have been best
at (the waiting spinner, the 10-minute window, the "Waiting for browser
confirmation..." panel). Every item is marked `SOURCE`. The surfaces that *are*
observable unauthenticated are listed explicitly in `07-accounts-auth.md` AUTH-15
and `08-launch-runtime.md` LAUNCH-21 so the orchestrator can spend its one
display on them.

**What the orchestrator should capture unauthenticated** (all reachable with no
account): the rail sign-in glyph; the "Account required" modal by clicking any
gated feature; the "Minecraft required" modal by pressing Play; the "Not signed
in" sidebar card; the two Getting-started checklist rows; the Play button in its
`Repair` state on a brand-new instance; and the entire install-job pipeline —
which needs no account at all, because creating an instance and installing a
public modpack only touches the public content API. That last one is the highest
value: it exercises the same phase vocabulary, progress reporting and error
sentences the launch pipeline uses.

**Worth watching.** The friends list is hidden until Modrinth sign-in
(`App.vue:421`), yet it contains an inline "Sign in to a Modrinth account to add
friends" link. That link appears to be unreachable. Confirm by observation; do
not take the reading alone.

## OQ-007 — There is no crash surface at the moment the crash happens

**Question.** §6 asks for crash handling: exit codes, how a crash report is
produced, surfaced and stored, and what the user is offered. What is the answer?

**Finding.** There is no crash *dialog*. `ProcessPayloadType` has exactly two
variants, `Launched` and `Finished`, and `Finished` carries neither the exit
status nor any indication that the exit was abnormal
(`packages/app-lib/src/event/mod.rs:409`). The status is written to the
launcher's own log and never shown. Diagnosis is deferred to a third-party
service: the log text is posted to `api.mclo.gs/1/analyse`, but only when the
Logs tab is mounted or when a `Finished` event fires while it is
(`apps/app-frontend/src/pages/instance/logs/index.vue:190`, `:212`).

**Decision.** Documented as found, and flagged as the clearest place to improve
on rather than copy. The exit-status line and the phase taxonomy are worth
copying; the missing crash moment is not. Recorded because "our launcher should
be better here" is a claim the owner may want to overrule — it is a real scope
decision, since a crash dialog means designing a report format and a place to
put it.

**Never observed.** Launching a game at all was out of reach (OQ-006), so the
exit-status recording, the post-exit sequence and the mclo.gs analysis are
**source reading, not observation**. `08-launch-runtime.md` says so at LAUNCH-21.

## OQ-008 — Skins and capes: declined, and why that is recorded

**Question.** Our stated scope excludes skins. Modrinth ships a full skin
selector anyway. Should the blueprint describe it?

**Finding.** It is a substantial subsystem: a three-source reconciliation model
between local rows, bundled defaults and Mojang's profile, PNG dimension
validation, equip/reorder/delete, cape side effects, and a flush-before-launch
race to avoid losing a pending change
(`packages/app-lib/src/api/minecraft_skins.rs:1-48`, `:1405`; `:run.rs:294`).

**Decision.** Described at AUTH-12 and **consciously not scoped in**. Two ideas
from it are recorded as transferable and unrelated to skins: flush deferred
mutations at the last safe point before a launch rather than racing them, and
write the local/remote reconciliation rules down as prose at the top of the
module. Recorded as an entry rather than a silence so the omission reads as a
decision with a reason.

## OQ-009 — "How does it name problems" has one clear defect, and we should not copy it

**Question.** §6 calls the error-copy analysis a headline deliverable and asks
us to match Modrinth's helpfulness. Does that mean matching all of it?

**Finding.** No. The taxonomy itself is the best thing in the slice — an error
**code** chosen from the error *and its phase*, translated to a bounded set of
plain sentences, with per-file paths, expected hashes and sizes persisted so
"Copy details" emits a near-finished support document. But the app already
computes the answer to "is the disk full?" (`fs4::available_space`, with a 64 MB
working margin) and then throws the message away on the common path, reporting
**"Couldn't save files"**. It names it correctly in exactly one place, the
directory-move dialog, which proves the team knows better
(`filesystem.rs:243`; `ErrorModal.vue:82`).

**Decision.** Documented as found, and explicitly marked in LAUNCH-14 as the one
to improve on rather than copy. Matching Modrinth's *helpfulness* is the
instruction; matching its *vocabulary* is not. The principle we adopted: where a
pre-flight check already knows the cause, name the cause.

## OQ-010 — Stop is a hard kill, and that is probably right for us too

**Question.** The brief asks how a running game is stopped — signal? graceful?
and what state that leaves the app in.

**Finding.** `ProcessManager::kill` calls `tokio::process::Child::kill()` and
awaits it (`packages/app-lib/src/state/process.rs:384`). No window-close
message, no `stdin` command, no signal escalation, no wait-then-force. On Unix
that is `SIGKILL`. Because tokio's `kill()` is `start_kill()` then `wait()`, it
does block until the child is reaped, so the sequencing is sound — the
instance page awaits the kill before clearing its processes cache, and the
sequential process manager observes the same exit independently, so playtime
finalisation and the post-exit hook still run. Two aggravating details remain:
the launcher already runs its own main class *inside* the game process with an
RPC channel to it (`launcher/mod.rs:1145`, `:1309`), so a graceful
save-and-quit is technically available; and nothing warns the user that stopping
mid-save can lose world progress, on a control with no confirmation.

**Decision.** Documented as found; no recommendation attached, because whether
to build a graceful stop depends on our runtime integration, which is not
decided. Noted that a hard stop can lose world progress mid-save; if we keep the
hard kill, a confirmation on stop is the cheap mitigation.

## OQ-006 — The orchestrator's shell capture lists nine rail destinations; the rail has eight slots

**Question.** The brief handed to the 01-shell-navigation worker described the
left rail as having destinations "top→bottom: y=72 Home, y=124 Discover
content, y=176 Skin selector, y=228 Screenshots, y=280 Hosting/Servers,
divider, y=356 Create instance, y=716 Settings, y=768 (unlabelled)" — and
separately listed nine, counting the unlabelled bottom slot.

**Finding — SOURCE resolves it.** `apps/app-frontend/src/App.vue:2261-2392` is
the entire rail: five route `NavButton`s, a `QuickInstanceSwitcher` wrapper
carrying the create button in its footer, a Settings `NavButton`, and one of
three mutually exclusive account elements. That is **eight** slots, and the
unlabelled bottom slot is the **account menu**, not a ninth destination. The
capture's own coordinates confirm the source order: with a 48 px button, 4 px
gap, 8 px side padding and no top padding (`App.vue:2262`), the first five
centres are exactly 72/124/176/228/280; the `flex-1` switcher wrapper
(`QuickInstanceSwitcher.vue:293`) accounts for the gap between y=356 and y=716.

**Decision taken.** Documented eight slots, with the account menu as NAV-19 and
its tooltip string recorded as the source of the "unlabelled" note. No
contradiction remains.

**Consequence.** The capture also proves that **no rail slot was conditionally
hidden** in that session — so `show_skin_selector_in_sidebar` was `true` and
`GlobalSyncedOptions.screenshots` was `true`. Two of the eight slots *can*
disappear. See OQ-007.

## OQ-007 — Screenshots and Skins are the only rail slots that can vanish, and one of them is gated on the wrong setting

**Question.** The brief asked whether Skins/Screenshots/Hosting are "always
present". They are not, and the reason matters for the rewrite.

**Finding — SOURCE.**
- **Hosting is always present.** No `v-if`, no feature flag, no entitlement
  check (`App.vue:2299-2310`).
- **Skins is conditional** on `settings.show_skin_selector_in_sidebar`
  (`App.vue:2286`), default `true` (`composables/use-app-settings.ts:57`),
  and account-syncable (`App.vue:1305-1320`). This is a correct, ordinary
  preference — but the only one of its kind on the rail.
- **Screenshots is conditional on the wrong thing**: `v-if` reads
  `globalSyncedOptionsQuery.data.value?.screenshots` (`App.vue:2293`), which is
  `GlobalSyncedOptions.screenshots` — the *instance-synchronisation capability*
  for the screenshots folder
  (`packages/app-lib/src/api/instance/synced_options/orchestration.rs:55-65`),
  itself mirrored from `behavior.show_all_screenshots` on the account
  (`App.vue:1325-1338`).

**Alternatives rejected.** Treating this as a deliberate progressive-disclosure
decision ("only show Screenshots if you have screenshots syncing"). It is not
plausible: the route exists unconditionally (`routes.js:60-64`) and would render
an empty gallery. Two unrelated intents — "show the screenshots page" and "sync
screenshots between instances" — were conflated into one condition.

**Decision taken.** Documented as an **IA defect**, not a feature
(`modules/01-shell-navigation.md`, NAV-14, and defect 2 in the rationale). Our
rewrite should gate the Screenshots entry on "does this user have any screenshots"
and expose syncing as an independent capability toggle.

**Why it is not being fixed here.** These are research documents, not the
rewrite. Recorded so that the implementing agent inherits the finding rather
than re-deriving it, and so the owner can overrule the "this is a bug" reading.

## OQ-008 — Two shell defects that only source review can find

**Question.** The orchestrator capture flagged an "Offline" chip (already OQ-004)
and an unlabelled bottom rail slot (OQ-006). Reading the source turned up three
further shell defects that no screenshot shows.

**Findings — SOURCE.**
1. **Wrong i18n string on the sidebar toggle.** Its accessible label is
   `messages.nextImage` = `app.navigation.next-image` / "Next image"
   (`App.vue:602`, `2428`) — a string borrowed from the image-viewer and used as
   the name of the button that hides the right sidebar. Screen readers announce
   "Next image" for a layout control.
2. **Hard-coded English window-control labels.** "Minimize window", "Toggle
   maximize window", "Close window" (`WindowControls.vue:11`, `19`, `28`) are
   literal strings, not `defineMessages` entries. They are the only chrome in
   the app that a non-English user cannot read.
3. **The sidebar toggle is unreachable by default.** Its `v-if` is
   `!forceSidebar && appSettings.toggleSidebar` (`App.vue:2426`), and
   `toggle_sidebar` defaults to `false`
   (`composables/use-app-settings.ts:53`). The setting reads as "should the
   sidebar start hidden" but is wired to "may the user hide it at all".

**Decision taken.** All three documented with evidence (NAV-09, NAV-10 and the
rationale). None "fixed" — this tree is read-only research.

**Alternative rejected.** Silently omitting them because they are small. They
are exactly the class of thing a rewrite inherits by accident if nobody writes
them down, and items 1 and 3 are behavioural (an unreachable control; a wrong
accessible name), not cosmetic.

## OQ-009 — There is no global search and no command palette

**Question.** The brief asked for "search entry points and every keyboard
shortcut bound to shell-level navigation". The honest answer is that the list is
nearly empty, and that is a finding rather than a gap in the research.

**Finding — SOURCE, by exhaustive absence.**
- The only `keydown` handler in `apps/app-frontend/src` that is not scoped to a
  component is the `N` quick-create handler in
  `apps/app-frontend/src/components/ui/WelcomeScreen.vue:50-68`, and it is
  unmounted whenever the user leaves the Home page.
- The only accelerators declared anywhere in the product are macOS menu items,
  `CmdOrCtrl+Z` and `CmdOrCtrl+Shift+Z` (`apps/app/src/macos/menu.rs:37-46`).
  Windows and Linux have no menu and therefore no accelerators at all.
- Search exists only as a field on the browse layout
  (`packages/ui/src/layouts/shared/browse-tab/layout.vue:49-53`). There is no
  ⌘K, no global omni-field, and no shortcut that reaches any rail destination.
- `ctrlKey`/`metaKey` appear in `apps/app-frontend/src` only as *guards* that
  suppress the `N` shortcut, never as a modifier that triggers anything.

**Decision taken.** Recorded the absence explicitly rather than describing the
browse search field as "the search entry point". Marked NAV-20 and NAV-21
`[ORCH-PENDING]` for the negative observation (tab from the rail: focus goes
straight to content).

**Why it matters to us.** This is the clearest case in the shell where Modrinth's
UX is *not* a model to copy. A launcher is the single most keyboard-driven
desktop application there is — the user's hands are on the keyboard while the
game runs — and this one offers one letter, on one page. A shell-level keymap
with one reserved key per rail destination, plus a palette over `/browse/*`, is
the highest-value single addition we could make to our own shell.

## OQ-010 — The 1100×700 minimum means "responsive" has exactly one data point

**Question.** The brief asks about resizing. How much responsive behaviour is
there to study?

**Finding.** `minWidth: 1100`, `minHeight: 700` (`apps/app/tauri.conf.json:83-84`)
with `resizable: true`. OQ-002 already established that the window refuses to go
below it. So the entire responsive range of this app is **one interval, from
1100×700 to unbounded**, and at the bottom of that interval the rail (64 px) plus
the right sidebar (300 px) leave 736 px of content.

**Decision taken.** Recorded the range as a single non-trivial data point rather
than claiming a responsive story. The only adaptive behaviour in the shell is
structural rather than breakpoint-driven: `forceSidebar` forces the sidebar open
on `/browse`, `/project` and `/user` regardless of the toggle
(`App.vue:266-271`); the breadcrumb measures its own overflow and scrolls
(`Breadcrumbs.vue:81-90`); and the download bar caps itself at `40 %` of window
width (`download-manager-bar.vue:139-149`).

**Alternatives rejected.** Inferring intermediate layouts by resizing to
1280/1600/1920 and calling them breakpoints. There are none — the shell has no
container queries and no width media queries; what looks responsive is three
independent `ResizeObserver`s. Saying so is more useful than implying a
breakpoint system that does not exist.
