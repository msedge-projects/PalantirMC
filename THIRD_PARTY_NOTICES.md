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
* **Licence**: GPL-3.0 for all three trees, and those terms are not this
  repository's to change. PalantirMC is proprietary -- `license` in `Cargo.toml`
  is `LicenseRef-Proprietary` by the owner's decision of 2026-10-08 -- and this
  entry is GPL-3.0 material still present while its replacement is written, so the
  GPL-3.0 terms apply to it, and to the combined work, until it is removed. The
  licence text travels with the code in
  `vendor/modrinth-app/LICENSE-GPL-3.0.txt`, with a copy in each of the three
  directories as upstream ships them; `NOTES.md` keeps the list of what remains
  and the work that removes each item.
* **Trademarks are not covered by that licence, and are a separate question.**
  Modrinth's name, wordmark and logo are its marks; GPL-3.0 grants rights in the
  code and not in the identity. The copies of that art in `assets/` and `ui/` are
  here as reference for what the port draws — sizes, boxes, colours, and which slot
  each picture fills — and the launcher draws its own mark and its own
  illustrations in those slots. `REFERENCE.md` records each place where it does.
* **Changes**: none. Every file is byte-for-byte upstream. An edit to any of them
  would be recorded here and in `UPSTREAM.md` before it was made, and the tree is
  merged with upstream rather than re-derived.

## Inter

* **What**: the five weights the whole interface is drawn in --
  `crates/palantir-desktop/assets/fonts/Inter-{400,500,600,700,800}.ttf`,
  embedded with `include_bytes!` from `crates/palantir-desktop/src/main.rs`'s
  `FONTS` and handed to iced through `Settings::fonts`. Modrinth sets its
  entire interface in Inter at weight 500 (`--font-weight-text`), so 400/500/
  600/700/800 is the ladder the reference's stylesheet asks for and 500 is the
  default this shell boots with.
* **From**: the reference's own files, not a re-derivation of them --
  `https://cdn.modrinth.com/fonts/inter/Inter-{Regular,Medium,SemiBold,Bold,
  ExtraBold}.woff?v=3.19`, the second `src` of each weight in
  `vendor/modrinth-app/packages/assets/styles/inter.scss`, fetched by
  `tools/make_fonts.py`. Modrinth's CDN serves each of them (HTTP 200,
  `font/woff`), and a WOFF is a container rather than a compression, so the
  script unwraps it into the plain sfnt cosmic-text 0.10 can read without a
  decompressor -- which matters, because the machine that builds this tree has
  no pip and the previous pipeline's `pyftsubset` could not run on it.
  * **These are Inter 3.19.** The `name` table's version string reads `Version
    3.019;git-0a5106e0b`, which looks like a different release and is not:
    Inter zero-pads the minor component to three digits, and the upstream
    `v3.19` release's own `Inter Desktop/*.otf` files carry that exact string
    and that exact git hash. The CDN's build is not a subset either -- 2505
    codepoints and 2548 glyphs, the same counts as `Inter Desktop/*.otf` in the
    upstream archive.
  * A pixel audit once attributed every label on the profile and hosting pages
    measuring one to five pixels narrower than the reference's to these faces
    being "3.019" rather than "3.19". It is not the typeface: `hmtx` is
    identical to the reference's for all 2505 codepoints in all five weights,
    and the residual delta has opposite signs at two different sizes, which one
    typeface cannot produce. `make_fonts.py` asserts the version string, the
    family, the PostScript name, `unitsPerEm` and the presence of the five
    tables a renderer needs before it writes a face, so a bad unwrap fails the
    tool rather than shipping a file that renders nothing.
* **Licence**: SIL Open Font License 1.1, the licence Inter is published under
  and one a proprietary application may redistribute: the OFL's condition is the
  copyright and licence records travelling with the font, not the licence of the
  application it is embedded in. The text travels with the faces in
  `crates/palantir-desktop/assets/fonts/OFL.txt`, which is upstream's own
  `LICENSE.txt` from the `v3.19` release verbatim -- OFL 1.1 asks for the
  copyright and licence records to accompany the font, and each face also
  carries them in its own `name` table (IDs 13 and 14), which
  `pyftsubset`'s `--name-IDs=*` had preserved and this pipeline gets for free
  by shipping whole faces.
* **Trademarks**: "Inter" is a trademark of Rasmus Andersson, which the OFL
  grant does not cover and which nothing here claims. The typeface is used
  because it is the typeface the interface being reproduced is drawn in.
* **Changes**: none to the font data. The WOFF unwrap reassembles the tables
  the container holds -- same bytes, recomputed table checksums and
  `head.checkSumAdjustment` -- and writes a plain sfnt. Nothing is subset,
  hinted, renamed or otherwise edited.

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

## Art adopted from the reference's own picture

* **What**: the art `ServerListEmpty.vue` and `ServerListEmptyPreview.vue` draw
  inside the *Servers* page's empty state, copied into
  `crates/palantir-desktop/assets/hosting/` and embedded with `include_bytes!`
  from `crates/palantir-desktop/src/pages/servers.rs`. Twelve PNGs, 1,160,895
  bytes in total:

  | File | Bytes | Where it is drawn | Box |
  | --- | --- | --- | --- |
  | `icon-texture.png` | 959,049 | the plate's three inner layers | `h-[6.25rem] w-[9.8125rem]` at `opacity-40` |
  | `josh.png`, `prospector.png`, `fetch.png`, `imb11.png`, `truman.png`, `boris.png`, `saya.png`, `michael.png` | 38,446 / 10,820 / 27,094 / 27,301 / 2,860 / 23,653 / 28,627 / 4,936 | one friend row each | `Avatar size="1.5rem" circle` |
  | `geometrically.png` | 13,639 | the invite toast | `Avatar size="2.25rem" circle` |
  | `modrinth-smp.png` | 23,919 | the toast's server line | `Avatar size="1.25rem"` |
  | `Pointer.png` | 551 | the badge on the *Prospector* row | `size-4` in a `size-8` badge |

* **Why**: the reference draws this page as a *picture* of its invite dialog --
  `ServerListEmptyPreview.vue` is `inert aria-hidden` -- and the picture is made of
  photographs. A port that drew the same picture with glyphs is a different
  picture: the eight rows are matched by photograph, so an account glyph cannot
  stand in for one of them. This adopts the picture rather than describing it.
* **From**: the same tree and the same commit as everything else above --
  `vendor/modrinth-app/`, <https://github.com/modrinth/code> at
  `8966b5e2e7951e83651fbebbf2fb6d7608a94a33`, taken from
  `ui/src/assets/welcome/icon-texture.png`,
  `ui/src/assets/servers/server-list-empty/*.png` and
  `ui/src/components/servers/server-list-empty/Pointer.png`. `modrinth-smp.png` is
  the Modrinth wordmark and `geometrically.png` is a photograph of a person; both
  are covered by the licence and *not* by it in the way the next paragraph sets
  out.
* **Licence**: GPL-3.0, the same licence as the vendored tree -- and not this
  repository's own, which is proprietary since 2026-10-08. These twelve files are
  GPL-3.0 material still present while their replacement is written, so those
  terms apply to them, and to the combined work, until they are gone; the text
  travels with the source in `vendor/modrinth-app/LICENSE-GPL-3.0.txt`, and
  `NOTES.md` names the work that removes them.
* **Trademarks, again**: Modrinth's wordmark is Modrinth's mark, and GPL-3.0 grants
  rights in the copy and not in the identity. The *Modrinth SMP* mark is here for
  the same reason the icon set above is: it is what the reference draws in that
  slot, and `REFERENCE.md` records which. `REFERENCE.md`'s standing rule -- that
  the launcher draws its own mark and its own illustrations -- is *not* followed
  for these twelve files, and this entry is where that departure is recorded.
* **Changes**: none. Every file is byte-for-byte upstream, verified with `md5sum`
  against its source. Every transform happens at draw time, in
  `servers.rs`: `avatar::Icon::of` and `Icon::circle` do the reference's `contain`
  fit and its corner masks, and the plate's texture is cropped to the window the
  reference's `object-cover` shows before it is composited -- none of it writes to
  the files.
