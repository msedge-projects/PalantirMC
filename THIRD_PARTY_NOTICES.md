# Third-party notices

This product includes software developed by other projects. Each entry names
what was taken, from where, under which licence, and where the licence text
lives.

These entries are not documentation of a history that could be tidied away:
every licence below requires its notice to travel with the code, in source and
in binary distributions alike. Adding an entry is part of taking the code.

## The Modrinth App — read as a specification, not taken as code

* **What**: nothing. No file, no function, no constant and no asset from this
  project is in this repository. Its source is read as the **specification** for
  what this launcher's shell draws — the exact tokens, box sizes, gaps, strings
  and timings that `REFERENCE.md` records — and the implementation here is
  independent Rust written against those numbers.
* **From**: <https://github.com/modrinth/code>, specifically
  `apps/app-frontend/src/App.vue`, its `components/ui/*`, and
  `packages/assets/styles/variables.scss`.
* **Licence**: GPL-3.0 for `apps/app-frontend`, and the brand assets are
  separately all rights reserved. That is why the port is a reading rather than a
  copy: using it as a spec keeps this repository's licensing unchanged, where
  lifting its Vue components would relicense the whole launcher. The distinction is
  load-bearing, so it is stated here rather than left to the code's history.
* **What this means in practice**: every number in `REFERENCE.md` that cites a
  line of its source is a *measurement of a public interface*, in the same sense as
  measuring its window's pixels — which is what the rest of that file does. No
  artwork, no icon file and no font from that project is redistributed here; the
  mark in the title bar, the illustrations on Home and Screenshots, and the loading
  page's wordmark are all this project's own.

## PandoraLauncher

* **What**: the `nbt` and `schema` crates, copied to `crates/nbt` and
  `crates/schema`; further PandoraLauncher crates (`auth`, `bridge`, `command`,
  `ftree`, `t`, `backend`) are adopted the same way as they are brought in.
* **From**: <https://github.com/Moulberry/PandoraLauncher>, of the same names.
* **Licence**: MIT — Copyright (c) 2025 Moulberry. Full text in
  [`licenses/PandoraLauncher-LICENSE.txt`](licenses/PandoraLauncher-LICENSE.txt),
  and repeated at the end of this file.
* **Changes**: recorded one by one, because every edit is a merge conflict
  waiting to happen if upstream moves.
  * `Cargo.toml` in each crate: dependency versions spelled out instead of
    inherited from Pandora's workspace manifest.
  * `lib.rs` in each crate: a doc comment carrying this notice, plus
    `#![allow(...)]` for the style lints upstream leaves at warn
    (`clippy::ptr_arg`, `len_without_is_empty`, `missing_transmute_annotations`
    for `nbt`; `match_like_matches_macro`, `collapsible_if`, `clone_on_copy`,
    `manual_strip`, `explicit_auto_deref`, `unnecessary_min_or_max` for
    `schema`). The code itself is left byte-identical so an upstream fix can be
    merged rather than re-derived; `clippy::correctness`, which this workspace
    denies, is deliberately not allowed.
  * `crates/schema/src/instance.rs`: the `cfg(not(unix))` arm of
    `get_shared_library_path_for_name` ignores its argument, so its parameter is
    `_name` there. No behaviour change.

### MIT licence text, as shipped by PandoraLauncher

MIT License

Copyright (c) 2025 Moulberry

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
