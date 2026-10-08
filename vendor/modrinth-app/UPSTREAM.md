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
| `tooling-config/` | `packages/tooling-config/tailwind` | 1 | 8.0 KB | GPL-3.0 |
| | | **1858** | **30.0 MB** | |

The one file in `tooling-config/` is `tailwind-preset.ts`, the Tailwind theme the
other two trees are written against. It is here because it is the *map* rather
than the values: every colour entry in it is `var(--...)`, so it says which
semantic name (`text-primary`, `bg-raised`) names which custom property
(`--color-text-primary`, `--surface-3`), while the values themselves are in
`assets/styles/variables.scss`, which was already here. Taking the file rather
than transcribing its aliases is the same rule the rest of this tree is under --
nothing about the reference is written down twice.

Its blob hash upstream at the pinned commit is
`91c3d520b1afa23bc8e5ea256282cfd25e792a0c`, and
`git hash-object vendor/modrinth-app/tooling-config/tailwind-preset.ts` answers
the same string.

Each directory carries its own upstream `LICENSE`, untouched; they are the
verbatim GPL-3.0 and differ from each other only in whitespace and line wrapping,
which is why the copies are byte-different and hash-different. `LICENSE-GPL-3.0.txt`
is a fourth copy of the same text, put at this directory's root so that the licence
governing the whole tree is visible without walking into the three.

Contents, by extension: 551 `.svg`, 533 `.vue`, 517 `.ts`, 109 `.json`, 70 `.png`,
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
| `packages/app-lib` (`theseus`) | — | the launcher backend: tokio, sqlx and about sixty dependencies, plus seven sibling path-crates. This launcher has its own backend, and the rewrite's decision was the lightest one that works rather than the most complete one that exists, so the reference is read for *what* the backend must do and not copied for *how*. `docs/superpowers/specs/2026-09-24-modrinth-native-rewrite.md` records that as a decision with its cost. |

`apps/app` *is* worth naming separately, because it is the one omission that a
reader of this port might expect to find: it is the Tauri host, and this launcher
has no Tauri. Everything it would have contributed — the window, the title bar, the
caption buttons, the drag regions, the window commands — is either iced's or
`crates/palantir-desktop/src/native.rs`'s, and where that code needs to know what
the Tauri side did, the reason is written at the call site.

## That it is verbatim, as a command rather than a claim

"Byte for byte" is checkable, so it is checked. Every file in this tree has the
same git blob hash as the same path upstream at the pinned commit:

```
# in the upstream clone
#   git ls-tree -r HEAD apps/app-frontend packages/ui packages/assets \
#     packages/tooling-config/tailwind \
#     | awk '{print $3, $4}' | sort > up.txt
# here
#   git ls-tree -r HEAD vendor/modrinth-app \
#     | awk '{print $3, $4}' \
#     | sed 's#vendor/modrinth-app/app-frontend/#apps/app-frontend/#;
#            s#vendor/modrinth-app/ui/#packages/ui/#;
#            s#vendor/modrinth-app/assets/#packages/assets/#;
#            s#vendor/modrinth-app/tooling-config/#packages/tooling-config/tailwind/#' \
#     | grep -v 'LICENSE-GPL-3.0.txt\|UPSTREAM.md' | sort > ours.txt
#   diff <(cut -f1,2 up.txt) <(cut -f1,2 ours.txt)
```

The two lists are 1857 lines each and the diff is empty — 1857 and not 1858
because `LICENSE-GPL-3.0.txt` is a fourth copy of the licence `app-frontend`
already carries and `UPSTREAM.md` is this file, neither of which exists upstream.
Matching *blob hashes* rather than file contents also settles the one thing a
content comparison would leave open on a Windows checkout: whether git rewrote the
line endings on the way in. It did not, and `.gitattributes` says why — `-text` on
`vendor/modrinth-app/**`, so no end-of-line conversion on checkout and no
normalisation on commit.

## Licence, and what adopting it means for this repository

All three trees are **GPL-3.0**. This repository's own terms are proprietary since
2026-10-08 — see `license` in `/Cargo.toml`, which is `LicenseRef-Proprietary` by
the owner's decision — and that decision does not reach in here: these files are
GPL-3.0 by their authors' grant, the obligation travels with them, and it is being
met where it is still owed. The licence text travels with the code here, the
provenance is this file, and `THIRD_PARTY_NOTICES.md` names it; `NOTES.md` keeps
the list of GPL-3.0 material that is still in the tree and the work that removes
it.

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
