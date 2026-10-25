# fastframe-scroll, vendored

`crmne/fastframe`'s scroll crate at tag **v0.6.0**, MIT, adopted the way
`vendor/cosmic-text` was: the source is the tag's own bytes, the manifest beside it
is rewritten to stand alone (upstream inherits everything from its workspace), and
the licence travels with it in `LICENSE-MIT.txt`.

| | |
| --- | --- |
| Repository | https://github.com/crmne/fastframe |
| Tag | `v0.6.0` |
| Crate | `crates/fastframe-scroll` |
| Files taken | `src/lib.rs` (11,637 B), `src/tests.rs` (10,331 B), `README.md` (2,887 B) |
| Blob hashes at the tag | `lib.rs` `dce526e263bf863d1b3a69ed89745d3aeed4ea79`, `tests.rs` `d37a575f5966233e11d74c30a48ae097c8a482d5` |
| Depends on | `egui` 0.36.1 only |

## Why vendored rather than a git dependency

`fastframe-scroll` is **not published on crates.io** (`cargo info fastframe-scroll`
answers "could not find"). A git dependency would make every build — CI, this
machine, a fresh clone — fetch a repository, which is the thing a launch build
should not need. Vendoring keeps the tree self-contained, which is the same reason
`cosmic-text` is here.

## What it does, and why this launcher wants it

A wheel notch scrolls 120 points rather than egui's 40; on Linux a touchpad's deltas
are scaled 1.8×, glided after the fingers lift, and held to the axis they started on.
It rewrites `InputState::smooth_scroll_delta` once per frame, so every `ScrollArea`
picks it up; nothing else changes.

That is the behaviour the reader asked for twice: "smooth as spotifast", where
spotifast is the same author's egui app using this crate.

## Edits

None. `src/` is byte-identical to the tag; the only file written here is
`Cargo.toml`, and its own comment says what changed. Every change to an adopted
crate has to be recorded here and in `THIRD_PARTY_NOTICES.md` — there are none yet.
