# What this directory is, and where every byte of it came from

This is a verbatim copy of the parts of the Modrinth App that draw its window,
taken so that this launcher's port can be read against the real thing instead of
against screenshots of it. It is **adopted third-party code**, not this project's:
no file in here has been edited, and none of it is compiled or shipped. The
launcher in `crates/` is independent Rust that draws the same window; this is the
specification it draws it from, kept locally so the port can be checked without a
network round trip per question.

Repo-wide rules that apply to it are in `AGENTS.md`: adopted code keeps its own
formatting and lints, every edit to it would be recorded in
`THIRD_PARTY_NOTICES.md`, and an adopted tree is merged with upstream later rather
than re-derived.

## Where it came from

| | |
| --- | --- |
| Repository | <https://github.com/modrinth/code> |
| Branch | `main` |
| Commit | `8966b5e2e7951e83651fbebbf2fb6d7608a94a33` |
| Commit date | 2026-09-20 13:33:59 +02:00 |
| Retrieved | 2026-09-21, with `git clone --depth 1` |

Reproduce it exactly:

```
git clone --depth 1 https://github.com/modrinth/code.git
git -C code checkout 8966b5e2e7951e83651fbebbf2fb6d7608a94a33   # depth 1 already is it
```

The commit is the pin. Nothing here is fetched by tag or by branch, because a
branch moves and every number in `REFERENCE.md` is a statement about one revision.

## What is here

| Directory | Upstream path | Files | Size | Licence |
| --- | --- | --- | --- | --- |
| `app-frontend/` | `apps/app-frontend` | 462 | 12.3 MB | GPL-3.0 |
| `ui/` | `packages/ui` | 833 | 12.4 MB | GPL-3.0 |
| `assets/` | `packages/assets` | 561 | 5.3 MB | GPL-3.0 |
| | | **1857** | **30.0 MB** | |

Each directory carries its own upstream `LICENSE`, untouched; they are the
verbatim GPL-3.0 and differ from each other only in whitespace and line wrapping,
which is why the copies are byte-different and hash-different. `LICENSE-GPL-3.0.txt`
is a fourth copy of the same text, put at this directory's root so that the licence
governing the whole tree is visible without walking into the three.

Contents, by extension: 551 `.svg`, 533 `.vue`, 516 `.ts`, 109 `.json`, 70 `.png`,
23 `.js`, 15 `.webp`, 11 `.scss`, 7 `.md`, 5 `.css`, 2 `.gltf`, 2 `.fbx` — plus the
four licences. `node_modules`, `dist` and `.turbo` are not present: they are build
output, they are git-ignored upstream, and `AGENTS.md` forbids committing build
output here too.

## What is deliberately not here

The upstream repository is 362 MB of packed history and 287 MB of working tree, and
most of it is not the launcher. What was left out, with the size it would have cost:

| Upstream path | Size | Why not |
| --- | --- | --- |
| `apps/frontend` | 135 MB | the website (modrinth.com), not the app |
| `packages/blog` | — | the blog, including 100+ MB of article video |
| `apps/app` | — | the Tauri shell: window creation, updater, the Rust side |
| `packages/moderation`, `packages/utils` | 1.2 MB | server-side and shared helpers this port does not draw |
| `apps/labrinth`, `apps/daedalus`, `apps/theseus` | — | the API, the docs and the auth service |

`apps/app` *is* worth naming separately, because it is the one omission that a
reader of this port might expect to find: it is the Tauri host, and this launcher
has no Tauri. Everything it would have contributed — the window, the title bar, the
caption buttons, the drag regions, the window commands — is either iced's or
`crates/palantir-desktop/src/native.rs`'s, and where that code needs to know what
the Tauri side did, the reason is written at the call site.

## Licence, and what adopting it means for this repository

All three trees are **GPL-3.0**. This repository is already `GPL-3.0-only` — see
`license` in `/Cargo.toml` — so this is a compatible adoption rather than a
relicensing of anything. What it does change is the obligation, and the obligation
is already being kept: the licence text travels with the code here, the provenance
is this file, and `THIRD_PARTY_NOTICES.md` names it.

The brand marks are **not** covered by that licence and are a separate question
from it. Modrinth's name, wordmark and logo are its trademarks, and a trademark is
not something GPL-3.0 grants a right to use — the licence covers the code, not the
identity. The copies of that art in `assets/` and `ui/` are here as *reference* for
what the port draws: sizes, boxes, colour, and which slot each picture fills. The
launcher that ships from this repository draws its own mark and its own
illustrations in those slots, and `REFERENCE.md` records every place where it does.

If a build is ever made that carries their art as its own identity, that decision is
the repository owner's and not the licence's to authorise. Keeping the art in a
clearly-named `vendor/` tree with this file above it is what makes that choice
visible and reversible rather than implicit.

## Removing it

One directory, no references into it from the build:

```
rm -rf vendor/modrinth-app
```

Nothing in `crates/`, `tools/`, the CI workflows or `Cargo.toml` reads this tree, so
its absence changes no test and no artifact. It is reference material that happens
to be large, which is the reason this section exists at all.
