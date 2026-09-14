# Third-party notices

This product includes software developed by other projects. Each entry names
what was taken, from where, under which licence, and where the licence text
lives.

These entries are not documentation of a history that could be tidied away:
every licence below requires its notice to travel with the code, in source and
in binary distributions alike. Adding an entry is part of taking the code.

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
