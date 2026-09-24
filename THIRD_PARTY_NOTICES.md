# Third-party notices

This product includes software developed by other projects. Each entry names
what was taken, from where, under which licence, and where the licence text
lives.

These entries are not documentation of a history that could be tidied away:
every licence below requires its notice to travel with the code, in source and
in binary distributions alike. Adding an entry is part of taking the code.

## The Modrinth App

* **What**: the parts of the Modrinth App that draw its window, copied verbatim
  into `vendor/modrinth-app/` — `apps/app-frontend` (462 files), `packages/ui` (833)
  and `packages/assets` (561), 1857 files and 30.0 MB in total, each with its own
  upstream `LICENSE` untouched. None of it is compiled or shipped: the launcher in
  `crates/` is independent Rust that draws the same window, and this tree is the
  specification it is ported against and checked against.
* **From**: <https://github.com/modrinth/code> at commit
  `8966b5e2e7951e83651fbebbf2fb6d7608a94a33` (2026-09-20), retrieved with
  `git clone --depth 1`. `vendor/modrinth-app/UPSTREAM.md` is the provenance: the
  pin, the command to reproduce it, a file count and a byte count per directory,
  the contents by extension, everything upstream that was *not* taken with the
  size it would have cost, and the one-line command that removes the tree.
* **Licence**: GPL-3.0 for all three trees. This repository is `GPL-3.0-only`
  already (see `license` in `Cargo.toml`), so this is a compatible adoption and not
  a relicensing of anything; the licence text travels with the code in
  `vendor/modrinth-app/LICENSE-GPL-3.0.txt`, with a copy in each of the three
  directories as upstream ships them.
* **Trademarks are not covered by that licence, and are a separate question.**
  Modrinth's name, wordmark and logo are its marks; GPL-3.0 grants rights in the
  code and not in the identity. The copies of that art in `assets/` and `ui/` are
  here as reference for what the port draws — sizes, boxes, colours, and which slot
  each picture fills — and the launcher draws its own mark and its own
  illustrations in those slots. `REFERENCE.md` records each place where it does.
* **Changes**: none. Every file is byte-for-byte upstream. An edit to any of them
  would be recorded here and in `UPSTREAM.md` before it was made, and the tree is
  merged with upstream rather than re-derived.
