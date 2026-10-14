# API surfaces — what the Modrinth App calls, and with what parameters

Every network surface of the app, cited to the code that builds the request.
Source is `modrinth/code` at tag `v0.21.9`, commit `6ebd75b6`. The consumer is
our Rust `palantir-net`, so each entry ends with a **mirror / skip** call.

The client is a typed wrapper: `packages/api-client/src/modules/labrinth/`.
Each method declares its `api`, `version`, `method` and `params`, and the
wrapper serialises them. That means **the parameter names below are the real
ones** — they are not inferred from a URL.

---

## Summary table

| Surface | Endpoint | Method | Mirror? |
| --- | --- | --- | --- |
| Search / Browse | `/search` | GET | **mirror** — the core of Discover |
| Project detail | `/project/{id}` | GET | **mirror** |
| Project dependencies | `/project/{id}/dependencies` | GET | **mirror** — install resolution |
| Project gallery | `/project/{id}/gallery` | GET | mirror |
| Project check | `/project/{id}/check` | GET | mirror |
| Projects by id | `/projects` | GET | mirror |
| Version list | `/versions` | GET | **mirror** |
| Single version | `/version/{id}` | GET | **mirror** |
| Version by hash | `/version_file/{hash}` | GET | **mirror** — integrity |
| Licenses | `/tag/license`, `/tag/license/{id}` | GET | mirror |
| Categories / loaders / game versions | tag endpoints | GET | **mirror** — facet lists |
| Auth (Microsoft) | device-code + token | POST | mirror |
| Auth (Modrinth) | OAuth implicit + loopback | GET/POST | mirror |
| User profile | `/user/{id}` | GET | mirror |
| Follows | `/project/{id}/follow` | POST | skip — no social |
| Collections | `/collection` | POST | skip |
| Friends | friends endpoints | GET | skip |
| Notifications | notifications endpoints | GET | skip |
| Analytics | `/analytics/...` | POST | **skip — out of scope** |
| App self-update | updater endpoint | GET | mirror |

---

## 1 — Search (Browse / Discover)

**Called by:** `packages/api-client/src/modules/labrinth/projects/v2.ts:81`
**Method & path:** `GET /search`
**Evidence:** `SOURCE projects/v2.ts:81-95`

**Parameters** (`SOURCE types.ts:1052-1064`):

| Name | Type | Notes |
| --- | --- | --- |
| `query` | string | free text |
| `facets` | `string[][]` | **JSON-stringified** before sending (`v2.ts:88`) |
| `index` | `'relevance' \| 'downloads' \| 'follows' \| 'newest' \| 'updated'` | the sort select |
| `offset` | number | pagination cursor |
| `limit` | number | page size — the UI's "View: 20" |
| `new_filters` | object | optional, sent only when present |

**Facet format** — the literal shape, from the doc comment at `v2.ts:74`:

```
facets: [['categories:optimization'], ['versions:1.17.1']]
```

So a facet is an **array of arrays of `key:value` strings**, and the outer array
is ANDed while the inner array is ORed. That is the whole filter model, and it is
why the UI can offer multi-select within a facet and AND across facets without
any special casing.

**Sort options** — exactly five, from `types.ts:1062`. `OBSERVED S-0003` shows
the UI's "Sort by: Relevance" default, which is `relevance`.

**For us: mirror.** This is the single most valuable endpoint in the study. Two
details to copy: the facet encoding (array-of-arrays, AND/OR) and the fact that
`facets` is **stringified by the client**, not by the caller — so a caller can
pass a structured value and never think about serialisation.

## 2 — Project detail and its tabs

| Tab | Endpoint | Evidence |
| --- | --- | --- |
| Description | `GET /project/{id}` | `SOURCE projects/v2.ts` |
| Versions | `GET /versions?project_id=…` | `SOURCE versions/v2.ts` |
| Version | `GET /version/{id}` | `SOURCE versions/v2.ts` |
| Gallery | `GET /project/{id}/gallery` | `SOURCE projects/v2.ts` |
| Dependencies | `GET /project/{id}/dependencies` | `SOURCE projects/v2.ts` |

**For us: mirror.** The dependency endpoint is the one that makes install
resolution possible, so it matters more than the display endpoints.

## 3 — Dependency types

**Evidence:** `SOURCE types.ts:1518`

```ts
export type DependencyType = 'required' | 'optional' | 'incompatible' | 'embedded'
```

Four values, and the distinction that matters is **required vs optional**: the
install flow asks the user about optional ones and installs required ones
without asking. `incompatible` and `embedded` are informational — they explain
why something is not being pulled in.

A second enum governs environment, `SOURCE types.ts:919`:

```ts
export type Environment = 'required' | 'optional' | 'unsupported' | 'unknown'
```

**For us: mirror both.** These two enums are the entire dependency model. Our
`palantir-loader` already carries `.mrpack` optional files through the review
(`UI-HANDOFF.md`); making the four types explicit is what lets the UI ask
instead of silently skipping.

## 4 — Version resolution and integrity

`GET /version/{id}` for metadata, and `GET /version_file/{hash}` to fetch a
file **by its hash** rather than by name. The hash-addressed fetch is what makes
a corrupted download detectable and a re-download resumable, and it is why the
content store can dedupe identical files across instances.

**For us: mirror.** Hash-addressed file fetch is the cheapest integrity win
available to us.

## 5 — Facet lists

Categories, loaders and game versions are fetched as tag lists
(`apps/app-frontend/src/pages/Browse.vue:250-258` calls `get_categories()`,
`get_loaders()`, `get_game_versions()` in parallel). Licenses come from
`GET /tag/license` and `GET /tag/license/{id}`
(`SOURCE packages/api-client/src/modules/labrinth/tags/v2.ts:11,32`).

**For us: mirror.** These are the option lists for every filter we will ever
build, and they change when Modrinth adds a loader — so they must be fetched, not
hardcoded.

## 6 — Auth

Two independent flows, documented in full in `modules/07-accounts-auth.md`:

- **Minecraft** — device-code grant, then PKCE Sisu exchange, in an in-app
  webview. `SOURCE apps/app/src/api/auth.rs:29`
- **Modrinth** — implicit OAuth grant with a **sealed token**: an ephemeral
  P-256 key pair plus a nonce, so a loopback listener cannot harvest the token.
  `SOURCE apps/app/src/api/mr_auth.rs:29`, `oauth_utils/auth_seal.rs`

**For us: mirror the Microsoft flow** (we already store and inject a session —
`UI-HANDOFF.md`). The Modrinth flow is the interesting one to read even if we
never ship it: sealing a token to an ephemeral key is the right answer to "a
desktop app has no client secret", and it is better than the common shortcut of
just opening a browser and hoping.

## 7 — What we should not call

| Surface | Why skip |
| --- | --- |
| Analytics (`/analytics/...`) | out of our scope; we ship no telemetry |
| Friends, follows, collections, notifications | need an account and a social graph we are not building |
| Servers/hosting | out of our scope; the reference spends three feature flags and five routes on it |
| Moderation, reports, organizations, teams | server-side product features, not launcher features |

## 8 — Rate limits, caching, CDN

- **Caching is a database table, not a file.** `cache` held 601 rows on a fresh
  install with no instances (`OBSERVED`, `data-model.md`) — almost entirely API
  responses. A response cache that survives restarts is why Browse feels instant
  on a second visit.
- **The content store is separate from the response cache.** `store/`,
  `store_operations`, `store_blobs`, `store_retained_refs` hold downloaded
  content with reference counting, so identical files are stored once and shared
  between instances.
- **Files are fetched by hash** (`/version_file/{hash}`), which is what makes the
  store dedupe correctly.
- **No client-side backoff was found** in the paths read. The app relies on the
  cache and on failing visibly rather than retrying aggressively — consistent with
  the mission's instruction not to scrape.

**For us: mirror the two-tier cache.** A response cache for metadata and a
content store for files, with the content store keyed by hash, is the difference
between an app that re-downloads and one that does not. It is also what makes
"clear cache" a safe action to offer.