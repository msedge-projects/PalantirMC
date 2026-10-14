# 07 — Accounts and sign-in

Two independent identities live in this launcher and they are never conflated:
the **Modrinth account** (a website session token, needed for discovery,
sharing, hosting, friends, sync) and the **Minecraft account** (a Microsoft/Xbox
token chain, needed to actually start the game). Either can exist without the
other; both can exist several-at-once.

All paths are relative to the pinned checkout of `modrinth/code` at
`6ebd75b68dd386cd4d74eee1cf2e96d945c937db` (`v0.21.9`).

## Second observation pass (2026-10-14)

The first pass documented both flows from source and listed, in AUTH-15, the
account-gated surfaces it could not see. A second pass created an instance with
**no account at all** (OQ-012), which proves the account-gate boundary from the
other side: **creating, installing and configuring an instance never asks for an
account.** The sole gate is at launch `[S-0100]` — the "Minecraft required"
modal — and the Modrinth-account features (Sharing, Hosting, friends) stay hidden
or empty until Modrinth sign-in, as source predicted. No credential of any kind
was created or used (OQ-013).

## Coverage table

| ID | label | type | evidence | status |
| --- | --- | --- | --- | --- |
| AUTH-1 | Modrinth sign-in entry points | button / dialog | `apps/app-frontend/src/App.vue:2389`, `.../ModrinthAccountRequiredModal.vue:1` | SOURCE |
| AUTH-2 | Modrinth sign-in state machine | state | `apps/app/src/api/mr_auth.rs:29`, `.../oauth_utils/auth_code_reply.rs:78` | SOURCE |
| AUTH-3 | Modrinth account switcher | menu | `apps/app-frontend/src/App.vue:1463` | SOURCE |
| AUTH-4 | Modrinth sign-out | button | `apps/app-frontend/src/App.vue:1425`, `packages/app-lib/src/api/mr_auth.rs:63` | SOURCE |
| AUTH-5 | Modrinth session store + refresh | state | `packages/app-lib/src/state/mr_auth.rs:16` | SOURCE |
| AUTH-6 | Minecraft sign-in state machine | state | `apps/app/src/api/auth.rs:29`, `packages/app-lib/src/state/minecraft_auth.rs:110` | SOURCE |
| AUTH-7 | Minecraft account card / multi-account | accordion | `apps/app-frontend/src/components/ui/AccountsCard.vue:1` | SOURCE |
| AUTH-8 | Minecraft token store + refresh | state | `packages/app-lib/src/state/minecraft_auth.rs:267` | SOURCE |
| AUTH-9 | Minecraft sign-in failure diagnosis | dialog | `apps/app-frontend/src/components/ui/minecraft-auth-error-modal/minecraft-auth-errors.ts:9` | SOURCE |
| AUTH-10 | "Minecraft required" gate | dialog | `apps/app-frontend/src/components/ui/minecraft-required-modal/MinecraftRequiredModal.vue:1` | SOURCE |
| AUTH-11 | Getting-started checklist | indicator | `apps/app-frontend/src/components/ui/onboarding-checklist/index.vue:44` | SOURCE |
| AUTH-12 | Skins and capes (scope note) | page | `packages/app-lib/src/api/minecraft_skins.rs:1` | SOURCE |
| AUTH-13 | Account-gated surfaces | — | see "Surfaces that need an account" | SOURCE |
| AUTH-14 | Friends presence channel | indicator | `packages/app-lib/src/state/friends.rs`, `apps/app-frontend/src/components/ui/friends/FriendsList.vue:345` | SOURCE |
| AUTH-15 | Live observation of any of the above | — | requires a real Microsoft / Modrinth account | **[ORCH-PENDING]** — never performed; the brief forbids creating or using one |

---

## AUTH-1 — Modrinth sign-in entry points
**Type:** button + dialog
**Nav path:** Left rail, lowest slot (a keyhole-style sign-in glyph). When accounts are already stored, the same slot is an avatar overflow menu instead. Also reachable from any `auth.requestSignIn(...)` call site, and from the Getting-started checklist.
**Evidence:** [SOURCE `apps/app-frontend/src/App.vue:2389`, `apps/app-frontend/src/App.vue:2328-2391`]
**Purpose:** There is exactly one "you are not signed in to Modrinth" affordance in the whole app, and it is in the left rail. Everything else that needs a Modrinth account routes through one shared modal rather than each feature inventing its own prompt. The rail button is deliberately three-way: a spinner while credentials load, an avatar menu when signed in, a plain sign-in glyph otherwise.
**Operation:** Rail button calls `requestSignIn()` → `modrinthLoginModal.showSigningIn('sign-in', false)` (`App.vue:1416-1423`). The modal offers two buttons — "Create an account" (`sign-up` flow) and "Sign in to Modrinth" (`sign-in` flow) — plus a "Get support" link. Choosing either flips the modal into its waiting state and starts the browser hand-off. On success the modal hides itself and resolves `true`; on dismissal it resolves `false` and the caller decides what to do.
**Controls:** flow = `sign-in | sign-up`; `addAccount` flag (appends rather than replaces); cancel via the modal's close control, which also cancels the in-flight backend attempt.
**Persistence:** none at click time.
**Rationale & notes:** The modal's own wording is the key design idea — "You'll need to sign into your Modrinth account before you can use this feature" — it names the blocker instead of failing silently at the feature. It also offers the *create account* path from the same modal, so a user who hit a gated feature and has no account is not sent to a website to find out.

---

## AUTH-2 — Modrinth sign-in state machine
**Type:** state
**Nav path:** rail button → modal → system browser → back to app.
**Evidence:** [SOURCE `apps/app/src/api/mr_auth.rs:29-70`; `apps/app/src/api/oauth_utils/auth_code_reply.rs:40-136`, `:203-314`; `apps/app/src/api/oauth_utils/auth_seal.rs:1-149`; `packages/app-lib/src/api/mr_auth.rs:14-60`, `packages/app-lib/src/state/mr_auth.rs:196-246`]
**Purpose:** The Modrinth side is an *implicit* OAuth grant: the website redirects a bearer token straight back. Because there is no client secret in a desktop app, the token is sealed to an ephemeral public key the app generates per attempt, so a loopback HTTP listener cannot be used to harvest it.
**Operation:** the real state machine, in order.

1. **`Idle`** — nothing in flight. `PENDING_AUTH` is `None`.
2. **`Preparing`** — the command generates an ephemeral P-256 key pair plus a 16-byte random nonce (`AuthSeal::generate`), then opens a loopback TCP listener on any loopback address and records the pending-auth slot (`AuthReplySession::start`). Starting a second attempt *cancels* the first (`auth_code_reply.rs:46-53`).
3. **`Opening browser`** — the URL handed to the system browser is the website's own sign-in page with `launcher=true`, `protocol=2`, the base64url DER public key, the nonce, `ipver` and `port` appended (`mr_auth.rs:102-129`). A `LocalhostSignIn` feature flag redirects sign-in and sign-up to `localhost:3000` instead of the production site (`packages/app-lib/src/api/mr_auth.rs:4-31`).
4. **`Waiting for browser confirmation…`** — the modal shows this state with a spinner, a Cancel button, and an "Open browser again" button. The backend is blocked in `reply.wait()`, which races two sources: the loopback listener, and a `modrinth://auth` deep link (`auth_code_reply.rs:78-95`). There is **no timeout and no polling** — it is purely event-driven.
5. **`Callback received`** — either path yields an auth code. Loopback replies are validated against the expected nonce and answered with a small local HTML page: "You have successfully signed in! You can close this page now." on a 200, or "Authorization code not found. Please try signing in again." on a 400 (`auth_code_reply.rs:289-311`). A deep link whose nonce matches an in-flight attempt completes it; one that does not is dropped, and if nothing is waiting the app emits a toast: "Couldn't finish signing in. Please try again." (`apps/app/src/api/utils.rs:262-275`).
6. **`Opening seal`** — the code is decrypted with AES-256-GCM under an HKDF-SHA256-derived key from the ECDH shared secret; the plaintext must be `{nonce}.{sessionToken}`, and the token must match `mra_` + 32–128 alphanumerics. Any mismatch is "Invalid sealed app sign-in session" (`auth_seal.rs:73-149`).
7. **`Storing`** — `finish_login_flow` swaps the code for the user record via `GET /v2/user`, writes `expires = now + 2 weeks`, upserts with `active = true`, marks the onboarding checklist, and reconnects the friends socket (`state/mr_auth.rs:204-223`, `api/mr_auth.rs:34-60`).
8. **`Done`** — the main window is re-focused and the credentials returned to the frontend, which re-reads the account and repopulates the profile (`mr_auth.rs:65-67`, `App.vue:1399-1414`).

**Cancellation** exists at two levels: the frontend calls `cancel_modrinth_login()` → `cancel_pending_auth()`, which fires the cancel channel and aborts the listener task; and hiding the modal calls the same thing (`ModrinthAccountRequiredModal.vue:199-212`). A previous in-flight attempt is implicitly cancelled by starting a new one. A cancelled attempt surfaces to the frontend as the error "Login canceled", which `App.vue:1403-1413` deliberately swallows — it is not shown to the user as an error.

**Controls:** `flow` (`sign-in`/`sign-up`), `add_account` (appends `add_account=true` so the website does not auto-switch away from the current session), Cancel, "Open browser again" (cancels and restarts with a fresh seal and nonce).
**Persistence:** see AUTH-5.
**Rationale & notes:** Two choices here are worth carrying over. First, the seal: putting the bearer token on a loopback HTTP endpoint is the classic mistake in desktop OAuth, and the nonce+sealed-key design closes it — the callback carries an opaque blob only the app that requested it can open, and a stale callback cannot finish a later attempt because the nonce differs. Second, "Open browser again" exists because the most common failure is the user closing the tab or the browser not being the foreground window; re-issuing with a fresh nonce is safer than reusing a possibly-leaked one.

---

## AUTH-3 — Modrinth account switcher
**Type:** menu (submenu inside the rail avatar overflow menu)
**Nav path:** left rail avatar → "Switch account" submenu → one row per stored account, plus "Add account".
**Evidence:** [SOURCE `apps/app-frontend/src/App.vue:1447-1485`, `:1577-1583`; `packages/app-lib/src/api/mr_auth.rs:78-108`]
**Purpose:** More than one Modrinth account is fully supported. The switcher exists so a user with a personal and an alt account can swap without signing out and back in.
**Operation:** every row in `modrinth_users` is listed with its username and a role glyph; the active one is marked `selected`. Choosing a row that is not already current calls `setActive(userId)`, which flips `active`, upserts, and reconnects the friends socket. Each row has a trailing remove control ("Remove account", red `X`). Below a divider sits "Add account", which re-enters the flow with `addAccount = true`. When the user is signed out but has stored accounts, the same submenu renders in place of the avatar, with the sign-in glyph as the trigger.
**Controls:** select; remove (with no confirmation dialog); add.
**Persistence:** `modrinth_users` rows; `active` flag.
**Rationale & notes:** Switching is **destructive and immediate**: it writes the new active row to SQLite, applies the new account's cached appearance theme, fetches the new account's preferences, then calls `window.location.reload()` (`App.vue:1499-1531`). A full webview reload rather than a reactive update is the honest choice given how much of the tree depends on `credentials` — but it means the switch feels heavy, and it discards any in-page state. Removing the account you are currently signed in to also reloads. The "Remove account" control carries no confirmation: for a token you can re-issue by signing in again, that is arguably right, but for an account you have just added it is a single mis-click from gone.

---

## AUTH-4 — Modrinth sign-out
**Type:** button
**Nav path:** left rail avatar menu → "Sign out" (red, last item).
**Evidence:** [SOURCE `apps/app-frontend/src/App.vue:1425-1428`, `:1584-1590`; `packages/app-lib/src/api/mr_auth.rs:62-69`]
**Purpose:** Ends the website session without deleting stored accounts, so signing back in is one click and other accounts are untouched.
**Operation:** `logOut()` → `logout()` → `ModrinthCredentials::deactivate_all()` (sets `active = FALSE` on **every** row — this is a sign-out of all Modrinth accounts, not just the current one) and disconnects the friends socket. Then the frontend reloads the window.
**Clears:** the active session on every stored Modrinth account row; the live presence socket; live notifications (`liveNotificationsEnabled` follows `!!creds?.session`).
**Leaves behind:** every row in `modrinth_users` including its `session_id` and `expires`; the cached per-account appearance preference in local storage; the instance database; the cached user records.
**Controls:** none.
**Persistence:** `modrinth_users.active = FALSE` for all rows.
**Rationale & notes:** "Sign out" and "Remove account" are genuinely different operations with genuinely different consequences, and the app keeps them distinct — sign-out is reversible, removal is not. But `deactivate_all` means signing out of your alt also signs you out of everything, which the single-item menu does not advertise. Because nothing is deleted, signing back in is a browser round-trip that the session token would otherwise have satisfied; the session tokens are left in the database in a deactivated state.

---

## AUTH-5 — Modrinth session storage and refresh
**Type:** state
**Nav path:** n/a (background).
**Evidence:** [SOURCE `packages/app-lib/src/state/mr_auth.rs:7-13`, `:16-62`, `:117-176`; `packages/app-lib/migrations/20240711194701_init.sql:79-87`; `packages/app-lib/src/state/db.rs:9-22`; `apps/app-frontend/src/App.vue:1351-1397`]
**Purpose:** Keep a website session across restarts without prompting, and recover silently when it lapses.
**Operation:** everything lives in one SQLite file, `app.db`, in the platform data directory (overridable with `THESEUS_CONFIG_DIR`), in the table:

```
modrinth_users(id TEXT PK, active INTEGER, session_id TEXT, expires INTEGER)
```

On every read of the active account (`plugin:mr-auth|get`), `get_and_refresh` compares `expires` to now; if expired it POSTs `/v2/session/refresh` with the old session as the `Authorization` header. Success replaces the session and sets a fresh two-week expiry; failure deletes the row and returns "not signed in" — silently. Separately, at startup the frontend validates the token with `GET /v2/user` and, on a **401**, removes the account, clears live notifications, and falls back to signed-out (`App.vue:1373-1388`). A network error during validation is treated as "still valid" (`catch → true`), so an offline start does not sign you out.
**Controls:** none.
**Persistence:** key `modrinth_users.session_id` is the bearer token itself; `expires` is a unix timestamp; `active` is the switcher's selection. Related: the whole Modrinth account is a single opaque bearer string — there is no refresh token, no scope, no user id column beyond `id`.
**Rationale & notes:** Refresh is *lazy*, not scheduled — it happens on read, so the app never holds a timer for it. Failure is silent by design: a dead website session should not interrupt anything, it should just quietly return the app to its signed-out shape. The 401-at-startup path is the one visible-ish consequence (the avatar becomes a sign-in glyph). The trade-off is that a session revoked from another device is only discovered at next launch or next `get()`. Storing the bearer token in plaintext SQLite is the app's choice; the whole point of the seal in AUTH-2 is protecting it in transit, not at rest.

---

## AUTH-6 — Minecraft sign-in state machine
**Type:** state
**Nav path:** sidebar "Playing as" card → "Sign in to Minecraft"; or the "Minecraft required" modal; or the skin selector's sign-in control.
**Evidence:** [SOURCE `apps/app/src/api/auth.rs:27-87`; `packages/app-lib/src/state/minecraft_auth.rs:110-198`, `:695-732`, `:822-1190`]
**Purpose:** Obtain a Microsoft/Xbox token chain that ends in a Minecraft services access token. This is a full interactive OAuth authorization-code flow with PKCE, carried out in an **in-app webview window** rather than the system browser — the one flow where this app does not use a loopback listener.
**Operation:** the states, in order.

1. **`Idle`** — no sign-in window.
2. **`Getting device token`** — generates (or reuses) a P-256 device key, persisted in `minecraft_device_tokens`, and POSTs a Proof-of-Possession-signed request to the Xbox device auth endpoint. The signature covers method, path, timestamp and body; every request in this chain is signed this way (`minecraft_auth.rs:1541-1600`).
3. **`Getting Xbox auth request`** — calls `sisu.xboxlive.com/authenticate` with the device token, a PKCE `S256` challenge, `prompt=select_account`, and reads the `X-SessionId` response header. Failure to read that header is its own error, "Error reading XBOX Session ID header".
4. **`Waiting for sign-in (webview)`** — a Tauri window titled "Sign into Modrinth" opens the returned Microsoft consent URL, always-on-top, 1000×700 (min 500×500), centred, with `UserAttentionType::Critical` so the OS flashes it (`auth.rs:41-61`).
5. **`Polling`** — the backend then polls `window.url()` **every 50 ms** for up to **10 minutes**, looking for a redirect whose URL starts with `https://login.live.com/oauth20_desktop.srf` and carries a `code` query parameter (`auth.rs:63-83`).
6. **`Cancelling / abandoned`** — if the window is closed (`window.title()` errors) the flow returns `None` immediately; if 10 minutes elapse the window is closed and `None` is returned. Both are silent: the frontend simply leaves the button enabled (`AccountsCard.vue:238-248`, `MinecraftRequiredModal.vue:112-128`).
7. **`Exchanging`** — `login_finish` re-reads the device token, exchanges the code + PKCE verifier for a Microsoft OAuth token pair, then runs Sisu authorize → XSTS authorize → Minecraft token (`/launcher/login`) → entitlements check.
8. **`Fetching profile`** — one `GET /minecraft/profile` to learn the real player UUID and name; failure here aborts with "Failed to fetch player profile".
9. **`Done`** — `Credentials { access_token, refresh_token, expires, active: true }` is upserted and the onboarding checklist is marked.

**Controls:** cancel = close the sign-in window. No explicit cancel button exists inside the app; the window's own close is the only affordance.
**Persistence:** see AUTH-8.
**Rationale & notes:** The 10-minute budget and 50 ms poll are a **polling loop**, unlike the Modrinth flow's event-driven listener — the webview URL has no change event. 10 minutes is generous (Microsoft codes are short-lived) and the 50 ms interval is cheap because it is only reading a window property. "Critical" user attention is the right escalation for a window that can open behind the main window. The flow always sends `prompt: select_account`, so a user with several Microsoft accounts is always asked rather than silently reusing a cached session — that is a deliberate privacy-over-convenience choice.

---

## AUTH-7 — Minecraft account card and multi-account switching
**Type:** accordion (sidebar card)
**Nav path:** right sidebar, "Playing as" section, above the friends list.
**Evidence:** [SOURCE `apps/app-frontend/src/components/ui/AccountsCard.vue:1-86`, `:238-265`; `packages/app-lib/src/api/minecraft_auth.rs:51-101`]
**Purpose:** Show which Minecraft account the next launch will use, and let a user with several (family members, alts) pick or add one without leaving the app.
**Operation:** Two shapes. With **no** accounts it is a flat card reading "Not signed in" above a single "Sign in to Minecraft" button that spins while the flow runs. With one or more it is a collapsed accordion titled with the selected account's name and the subtitle "Minecraft account", showing a 36 px avatar; expanding it lists every account as a radio row with a 24 px avatar and a red trash button, plus an "Add account" button at the bottom. Selecting a row calls `set_default_user` and re-reads everything. The trash button calls `remove_user`; if that leaves nothing selected, the first remaining account is auto-selected. The card re-reads itself whenever a `process` event reports `launched`, so the avatar refreshes after each launch.
**Controls:** sign in; select; add; remove (no confirmation).
**Persistence:** `minecraft_users.active` is the selection.
**Rationale & notes:** Avatars come from `mc-heads.net`, falling back to a bundled Steve image — a third-party avatar service is a privacy dependency the app takes freely. The auto-select-next-account on removal is good: it means removing the last-but-one account does not silently break launching. Removal has no confirmation, and unlike Modrinth sign-out this one **deletes** the row including both tokens, so the cost of a mis-click is a full re-authentication. The accordion is collapsed by default (`:open-by-default="false"`), which keeps the sidebar quiet but means the active account name is the only always-visible state.

---

## AUTH-8 — Minecraft token storage and refresh
**Type:** state
**Nav path:** n/a (background).
**Evidence:** [SOURCE `packages/app-lib/src/state/minecraft_auth.rs:200-213`, `:267-316`, `:467-512`, `:593-652`, `:734-819`; `packages/app-lib/migrations/20240711194701_init.sql:52-77`]
**Purpose:** Keep a Minecraft session alive for weeks without re-authenticating, and survive transient network failure.
**Operation:** two SQLite tables in `app.db`:

```
minecraft_users(uuid PK, active, username, access_token, refresh_token, expires)
minecraft_device_tokens(id=0, uuid, private_key, x, y, issue_instant, not_after, token, display_claims)
```

The **refresh token** is the durable one; the **access token** is the short-lived Minecraft services token passed to the game. `Credentials::refresh` fires when `expires` is within **5 minutes** of now — the margin exists so the game has a valid token for its whole startup and to absorb clock skew (`minecraft_auth.rs:274-279`). It re-runs the refresh → Sisu → XSTS → Minecraft-token chain and re-upserts. `get_default_credential` (called on every launch) tolerates two failure classes specially: a **connect or timeout** error returns the stale-but-present credentials so an offline launch can still proceed, whereas an **`invalid_grant`** response deletes the account, promotes another one to active, and returns "no credentials" — which the frontend turns into the "Minecraft required" modal. The device key pair persists so the Proof-of-Possession identity is stable across restarts and the Xbox device token can be renewed rather than re-created.
**Controls:** none.
**Persistence:** the two tables above; keys `access_token`, `refresh_token`, `expires`, and a private key PEM.
**Rationale & notes:** The three-way handling of refresh failure — tolerate network, delete on revocation, propagate anything else — is the careful part and is easy to get wrong; getting it wrong either locks users out on a flaky connection or leaves them stuck in a retry loop against a revoked grant. The 5-minute skew margin is a good default most launchers copy. Note the serializer: when credentials are handed to the frontend, the backend opportunistically fetches the live Mojang profile so the avatar is correct, handling all three Tokio runtime flavors to do it (`minecraft_auth.rs:655-693`) — clever, and also a network call on a hot path.

---

## AUTH-9 — Minecraft sign-in failure diagnosis
**Type:** dialog
**Nav path:** surfaces automatically on any Minecraft-auth failure routed through `handleSevereError`.
**Evidence:** [SOURCE `apps/app-frontend/src/components/ui/minecraft-auth-error-modal/MinecraftAuthErrorModal.vue:1`; `.../minecraft-auth-errors.ts:9-200`; `apps/app-frontend/src/composables/use-error.js:28-36`]
**Purpose:** Turn an opaque reqwest/Xbox error string into "what we think happened" plus numbered fix steps. This is the app's single best piece of error UX and the model to copy.
**Operation:** The dialog is titled "Sign in Failed" and opens with a warning that the cause "may be due to account restrictions or regional limitations". Below it, the raw error message is matched against **16 ordered rules**, each with a `whatHappened` sentence and a numbered `stepsToFix` list. Matching is by substring on an XSTS/Xbox error code, substring on a Rust `ErrorKind` debug fragment, or a custom predicate. The rules cover, in order: refresh-token expiry; a clock/timezone skew (`SisuAuthenticate`); HTTP 429 rate limiting during token exchange; HTTP 5xx from Minecraft auth; a missing Java profile (recent purchase / wrong Microsoft account); DNS/proxy/firewall/VPN connectivity failure (matched on six Microsoft-owned domains); then the Xbox XSTS codes — `2148916222` UK age verification, `…233` no Xbox profile/owns no Minecraft, `…235` region unsupported, `…236`/`…237` South Korean adult verification, `…238` underage and unlinked from a family group, `…227` suspended, `…229` restricted from online play, `…234` ToS not accepted — and finally a generic `XstsAuthorize` fallback. Unmatched errors get an honest "Unknown error" panel that says the app can't recommend specific steps and points at the official Minecraft login page. Every action button is available: "Contact support", "Sign in again" (re-runs the whole flow inline), and a collapsible "Debug information" section with a copy button.
**Controls:** Sign in again; Contact support; expand/collapse debug; copy debug info.
**Persistence:** none.
**Rationale & notes:** Every rule's `whatHappened` names a *cause*, not a symptom, and every `stepsToFix` is something the user can do without leaving the app except where it genuinely cannot be. Links are injected as HTML (`v-html`) so a step can carry an official source. Two rules deserve specific credit: the clock-skew one, which correctly identifies the single most misdiagnosed Xbox failure, and the "Failed to fetch player profile" one, which is the exact confusion a first-time buyer hits.

Two structural weaknesses. First, **the older diagnosis is dead code, not a fallback.** `ErrorModal.vue:58-72` still carries a complete hard-coded explanation for the same class of failure ("Try another Microsoft account", "Using PC Game Pass, coming from Bedrock, or just bought the game?") — but the router at `use-error.js:28-36` intercepts *both* the `Minecraft authentication error:` prefix and any message `findMinecraftAuthError` matches, and routes them to this modal before `ErrorModal` is ever consulted. So the old copy is unreachable in normal operation and the app has exactly one live diagnosis path. That is the right outcome and a good migration to imitate: intercept by prefix *and* by a structured matcher, then delete the old copy rather than leaving two explanations to disagree. Second, **matching is string-based against Rust `Debug` output** — `'Failed to deserialize response to JSON during step RefreshOAuthToken:'` is a Rust error's `Display` string, not a typed discriminant. A Rust-side wording change silently degrades every affected diagnosis to "Unknown error", with no test to catch it. Our implementation should match on a typed error code, with the human sentence attached at the point the error is constructed.

---

## AUTH-10 — "Minecraft required" gate
**Type:** dialog
**Nav path:** appears over any surface when a launch is attempted with no usable Minecraft account.
**Evidence:** [SOURCE `apps/app-frontend/src/components/ui/minecraft-required-modal/MinecraftRequiredModal.vue:1`; `apps/app-frontend/src/composables/use-error.js:18-27`; `packages/app-lib/src/error.rs:149-150`; `packages/app-lib/src/api/instance/run.rs:55-57`]
**Purpose:** Convert the raw backend refusal "User is not logged in, no credentials available!" into a clear, actionable gate.
**Operation:** The error router inspects any error message for "user is not logged in" (and a second, now-unused string "cannot play instance since minecraft is required" — `grep` finds no producer of that string anywhere in the tree) and, instead of the generic error modal, opens this dialog: heading "Minecraft required", subheading "Sign in to a Microsoft account", body "You need a Microsoft account that owns Minecraft before you can launch and play.", with a "Get support" link, a "Sign in to Microsoft" button, and a footer "Don't have an account? Get Minecraft" linking to the store page.
**Controls:** Sign in to Microsoft (runs AUTH-6 and refreshes the sidebar card on success); Get support; Get Minecraft.
**Persistence:** none.
**Rationale & notes:** This is the correct shape for an auth gate: it states the requirement in the user's terms, gives the one action that resolves it, and — importantly for a launcher — handles the "I don't own the game" case explicitly rather than leaving the user to conclude it themselves. Note the copy says "owns Minecraft", which is the accurate constraint (the app checks entitlements during sign-in and refuses accounts that do not).

---

## AUTH-11 — Getting-started checklist
**Type:** indicator (sidebar accordion)
**Nav path:** right sidebar, above everything else.
**Evidence:** [SOURCE `apps/app-frontend/src/components/ui/onboarding-checklist/index.vue:44-63`; `packages/app-lib/src/state/onboarding_checklist.rs:9-45`; `apps/app-frontend/src/App.vue:420-421`, `:2515-2519`]
**Purpose:** Make the two sign-ins and the first instance discoverable instead of leaving a new user to hunt for them.
**Operation:** Three rows — "Create first instance", "Sign in to Minecraft", "Sign in to Modrinth" — each a button that runs the corresponding action directly (`create-instance` → the creation modal; `login-minecraft` → the sidebar card's `login()`; `login-modrinth` → the rail's `signIn`). Completion state is three booleans in `onboarding_checklist`; a completed row is struck through and disabled. The whole checklist hides automatically once all three are true. It also gates two other sidebar blocks: the Minecraft account card is `v-show="hasLoggedIntoMinecraft"` and the friends list is hidden until Modrinth sign-in.
**Controls:** three task buttons.
**Persistence:** `onboarding_checklist` row (`has_created_instance`, `has_logged_into_minecraft`, `has_logged_into_modrinth`, `show_checklist`).
**Rationale & notes:** Tying the two account sign-ins to explicit checklist rows is a good solution to the "where do I sign in" problem — most launchers bury it. Automatically hiding the accounts card until Minecraft sign-in is unusual and slightly risky: a user who signed in during a previous session but whose flag was lost would see no account card at all. It also means the friends list is literally invisible until Modrinth sign-in, which is why the "Sign in to add friends" affordance inside the list (`friends.sign-in-to-add-friends`) can only ever be seen by a user who has already signed in — an inconsistency worth noting rather than copying.

---

## AUTH-12 — Skins and capes (scope note)
**Type:** page (full-screen `/skins` route)
**Nav path:** left rail (visibility is a setting, `show_skin_selector_in_sidebar`) → route `/skins`, "Skin selector".
**Evidence:** [SOURCE `packages/app-lib/src/api/minecraft_skins.rs:1-48`, `:266-306`, `:528-1431`; `apps/app-frontend/src/pages/Skins.vue:1`; `packages/app-lib/src/api/instance/run.rs:294`]
**Purpose:** Full Mojang-profile skin management: list, upload, equip, reorder, delete; the model variant (classic/slim) and the equipped cape are set as a side effect of choosing a skin card.
**Operation:** The module's own header comment describes the data flow in detail — skins come from three sources (saved rows in the local DB, bundled Minecraft defaults, and the active Mojang skin), Mojang is the source of truth for what is equipped, and the local DB caches. A pending skin change is flushed to Mojang immediately before every launch (`run.rs:294` → `flush_pending_skin_change`), so a selection made in the UI is never lost by a launch that raced it. The page needs a Microsoft account; signed out it shows a "Sign in to Microsoft" control. Rendering is by head-crop URL plus a local `getPlayerHeadUrl` helper.
**Controls:** n/a — deliberately out of our scope.
**Persistence:** `custom_minecraft_skins` table plus `saved_skin_order`; the profile cache is in-memory only.
**Rationale & notes:** **We consciously decline this surface.** It is a substantial subsystem — a three-source reconciliation model, PNG validation (`InvalidSkinTexture` for anything not 64×64 or 64×32), Mojang round-trips, a "preserve the current skin before changing it" safety rule, and a flush-before-launch race to get right — for a feature that has nothing to do with running mods. Recorded here so the omission is a decision with a reason, not an oversight. The two transferable ideas are unrelated to skins: (a) flush deferred mutations at the last safe point before a launch, rather than racing them; (b) when a local cache can disagree with a remote source of truth, write down the reconciliation rules as prose at the top of the module.

---

## AUTH-13 — Account-gated surfaces
**Type:** —
**Nav path:** —
**Evidence:** [SOURCE `packages/app-lib/src/api/instance/run.rs:55-57`; `apps/app-frontend/src/App.vue:2328-2391`, `:2523-2544`; `apps/app-frontend/src/pages/hosting/manage/Index.vue:152-164`; `apps/app-frontend/src/components/ui/friends/FriendsList.vue:345-350`; `apps/app-frontend/src/pages/instance/share/index.vue:396`]
**Purpose:** The orchestrator needs to document each of these surfaces *unauthenticated*, because they change shape completely once signed in.
**Operation:** Gated on a **Minecraft** account:

- **Launching anything.** `run()` requires `Credentials::get_default_credential()` and errors `NoCredentialsError` otherwise. Surfaces: instance page Play button, instance context menu, Modrinth-server "Play server".
- **The skin selector** (`/skins`) and anything that reads or writes skins/capes.
- **Shared-instance membership management** as a non-owner (invites, reports) — `packages/app-lib/src/api/instance/shared/client.rs:258`, `:621`, `:887`, `:967`.

Gated on a **Modrinth** account:

- **Friends list and presence** — the socket only connects when a session exists; the empty state is the inline "Sign in to a Modrinth account to add friends and see what they're playing!" link.
- **Instance sharing / unpublishing / linked shared instances** — `/instance/:id/share` calls `auth.requestSignIn(...)`.
- **Modrinth Hosting** (`/hosting/manage/`, `/hosting/manage/:id`) — `resolveViewer()` returns a null viewer with no credentials; the Intercom support widget is disabled without a session.
- **Account settings** — the settings modal's Account category (Profile, Social, Privacy) is gated on `auth.user`; both `AccountProfileSettings.vue:3` and `AccountSocialSettings.vue:3` render a signed-out placeholder.
- **Live notifications**, Modrinth+ upsell, ads consent management (`adConsentAvailable` requires credentials to have loaded).
- **Playtime reporting** — a Modrinth-side analytics POST, skipped silently for unlinked instances.

**Available with no account at all:** the Home/welcome screen, importing an instance from another launcher, creating and configuring instances, installing mods from Browse (the content API is public), the Files / Worlds / Logs / Screenshots tabs, browsing Modrinth project and user pages, and reading the Settings modal (every non-Account tab).
**Controls:** n/a.
**Persistence:** n/a.
**Rationale & notes:** The split is clean and worth copying: **discovery is public, identity is not.** A signed-out user can do almost everything except start the game or touch another human's data. The one rough edge is that the gate is enforced in several places with different messages — "Minecraft required", "Account required", "You need to sign in to Modrinth", an inline link in the friends list — rather than one consistent pattern.

---

## AUTH-14 — Friends presence channel
**Type:** indicator
**Nav path:** right sidebar, below the accounts card (hidden until Modrinth sign-in — see AUTH-11).
**Evidence:** [SOURCE `packages/app-lib/src/api/mr_auth.rs:110-123`; `packages/app-lib/src/api/mr_auth.rs:56-68`; `apps/app-frontend/src/components/ui/friends/FriendsList.vue:62-71`]
**Purpose:** Show which friends are online and what they are playing, and publish your own status so they can see you.
**Operation:** A websocket connects with the active session, publishes status on launch (`friends_socket.update_status(Some(instance.name))` in `run.rs:1263-1266`) and clears it on exit (`process.rs:989`). The socket is disconnected and reconnected on every Modrinth sign-in, sign-out, account switch, account removal, and token loss.
**Controls:** n/a.
**Persistence:** none client-side.
**Rationale & notes:** Because presence rides on the *Modrinth* session, a user can be signed into Modrinth and playing without any friends visibility — and a user who signs out of Modrinth while a game is running silently loses their presence. Sign-in, sign-out, switch and remove all funnel through one `reconnect_friends` helper whose errors are only `tracing::warn!`-ed, so a failed reconnect is invisible: the friend list quietly stays empty. Worth being deliberate about in our implementation rather than inheriting by accident.

---

## AUTH-15 — Live observation of any sign-in surface
**Type:** state
**Nav path:** every AUTH item above.
**Evidence:** [ORCH-PENDING] — the brief forbids creating or using any real Microsoft, Minecraft or Modrinth account or credential, so no sign-in flow, no token refresh, no account switch and no sign-out was ever executed. Every statement above is `SOURCE`. Items marked `[ORCH-PENDING]` below are the ones that need an account:
- the visual state of the Modrinth sign-in modal in both its idle and waiting shapes,
- the browser hand-off and the loopback/deep-link return (would require driving a real browser),
- the "Sign into Modrinth" webview window and its 50 ms polling,
- the "Sign in Failed" diagnosis dialog with a real Xbox error code,
- the multi-account sidebar card with two or more accounts,
- the account switcher submenu with two or more Modrinth accounts.
**Purpose:** n/a
**Operation:** n/a
**Controls:** n/a
**Persistence:** n/a
**Rationale & notes:** What *is* observable unauthenticated, and should be captured by the orchestrator: the rail sign-in glyph in its idle state, the "Account required" modal in its pre-click state (reachable by clicking any gated feature), the "Minecraft required" modal (reachable by pressing Play with no account), the "Not signed in" sidebar card, the two Getting-started checklist rows, and the "Sign in to a Modrinth account to add friends…" empty state — note that per AUTH-11 that last one is currently *unreachable*, because the friends list is hidden until sign-in. That contradiction is worth confirming by observation rather than by reading alone.