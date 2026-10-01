# Third-party notices

This product includes software developed by other projects. Each entry names
what was taken, from where, under which licence, and where the licence text
lives.

These entries are not documentation of a history that could be tidied away:
every licence below requires its notice to travel with the code, in source and
in binary distributions alike. Adding an entry is part of taking the code.

## The Modrinth App

* **What**: the parts of the Modrinth App that draw its window, copied verbatim
  into `vendor/modrinth-app/` — `apps/app-frontend` (462 files), `packages/ui` (833),
  `packages/assets` (561) and `packages/tooling-config/tailwind/tailwind-preset.ts`
  (1 file), 1858 files and 30.0 MB in total, each with its own upstream `LICENSE`
  untouched. None of it is compiled or shipped: the launcher in `crates/` is
  independent Rust that draws the same window, and this tree is the specification
  it is ported against and checked against.
* **Read at generation time, not at build time**: two tools compile parts of that
  tree into Rust, and neither is part of the build.
  * `tools/gen_theme.py` reads the palette, the lengths, the curves and the motion
    out of the stylesheets into `crates/palantir-desktop/src/theme_gen.rs`.
  * `tools/gen_icons.py` reads the 313 SVGs in `assets/icons` into
    `crates/palantir-desktop/src/icons_gen.rs` — path geometry, stroke widths and
    opacities, as data the toolkit strokes. The icons themselves descend from
    Lucide, which is ISC-licensed; they are taken here from the Modrinth App's
    copy, which is what the vendored tree and its `LICENSE` cover.
  * `tools/gen_text.py` reads the two English locales —
    `apps/app-frontend/src/locales/en-US/index.json` and
    `packages/ui/src/locales/en-US/index.json` — into
    `crates/palantir-desktop/src/text_gen.rs`, 3846 messages of the interface's
    copy, compiled out of ICU MessageFormat. These are the reference's sentences
    and not ours: what that file emits is the same words, and the keys it names
    (`app.skins.section.modrinth-pride`, `browse.no-results`, …) are the
    reference's own. The five ICU constructs the reference does not use —
    `date`, `time`, `list`, `selectordinal` and apostrophe-quoted literals — are
    refused with the key that used one rather than approximated into something
    plausible. The strings that are *not* in a locale (`ui/src/utils/search.ts`'s
    five sort names) are recorded in the page that draws them, with the file they
    came from, rather than being invented here.

  All three generated files are this project's own work — a different language, a
  different shape, arcs and smooth curves resolved to cubics rather than copied,
  3846 sentences compiled into a Rust enum rather than shipped as JSON — but
  every value in them came from theirs, so all three are named here and each
  generator names its sources at the top of what it writes.
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

## cosmic-text

* **What**: the text engine the interface is shaped by -- vendored **and patched**
  at `vendor/cosmic-text/`. `iced`'s text stack is `cosmic-text` 0.10.0 (through
  `iced_graphics` 0.12.1), and what travels here is that crate's `src/` (20 files,
  241 KB), its `Cargo.toml` with the `[[bench]]` and dev-dependencies sections
  removed because neither directory travels with it, and both licence texts.
* **Why it is source rather than a registry dependency**: two files are edited, and
  the edits are why a Chinese project page draws words instead of tofu.
  * `src/font/system.rs` gains `FontSystem::get_fallback_font_matches` -- every face
    in the database, nearest weight first.
  * `src/shape.rs`'s `shape_run` builds its fallback candidates out of the faces
    that match the request exactly, followed by that list.
  Upstream asks `Attrs::matches` for a fallback candidate, and that is
  `face.post_script_name.contains("Emoji") || (style, weight and stretch all
  equal)`. The weight has to be *equal*, so a run at 500, 600 or 800 cannot reach a
  face published at 400: Windows ships Microsoft YaHei at 400 and 700 and Yu Gothic
  at 500, and the measurement taken through this engine paints four common
  simplified-only hanzi -- 简 戏 组 载 -- as `.notdef` at 500 while the rest of the
  line comes out of a Japanese face. The other half of the same defect is iced's,
  not this crate's: `iced_core`'s `Text::new` sets `shaping: Shaping::Basic`, which
  never asks for a fallback at all, and `crates/palantir-desktop/src/ui.rs`'s
  `text` is where `Shaping::Advanced` is turned on for every string the shell
  draws.
* **From**: <https://crates.io/crates/cosmic-text> 0.10.0, the version `Cargo.lock`
  already pinned. `Cargo.toml`'s `[patch.crates-io]` is what makes this copy the
  one `iced` compiles against, and the vendored `Cargo.toml`'s header names the two
  edited files.
* **Licence**: MIT OR Apache-2.0, both texts travelling with the source as
  `vendor/cosmic-text/LICENSE-MIT` and `vendor/cosmic-text/LICENSE-APACHE`.
* **Changes**: exactly the two edits above. The patch is meant to be merged
  upstream rather than carried -- offering the whole database to a fallback pass is
  what a fallback pass should do -- and a later `iced` that asks for a newer
  `cosmic-text` would have to bring it here again.
