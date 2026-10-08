# Build PalantirMC 2.0: a clean-room Minecraft launcher, our licence, our theme

This document **is the prompt**. Paste it into a fresh agent session as-is. It is
written so that the session needs nothing else: not this repository's history,
not the old tree, not the old assumptions.

Read the whole thing before writing any code. Two of the sections (the clean-room
rules and the audit) are the deliverable; the rest is the app.

---

## 1. Mission

Build a **Minecraft launcher for Windows, in Rust with iced**: instances, a
Modrinth-backed content browser, a launcher-engine (downloads, metadata, Java),
and a settings dialog. Ship it as **our own product, our own design, under our
own licence.**

There is an existing implementation at `MSedgeMC/PalantirMC`. **It is not your
base and you must not read it.** It is a GPL-3.0 derivative work of the Modrinth
App: its icons, its interface sentences, its colour tokens and its documentation
all come from that project's source, and its own README says so. Any file taken
from it stays GPL-3.0, and GPL-3.0 is a copyleft licence: one GPL file means the
whole combined work must be GPL. **The new repository must therefore start
empty and be written from this document, not adapted from that one.**

What you may do: run a launcher as a *black box*, look at it, measure what it
draws, and read public specifications (the Minecraft launcher's version JSON and
asset-index formats, Prism/MultiMC's `instance.cfg` format, the `.mrpack` format,
the Modrinth API docs, Mojang's piston-meta docs). Functionality, information
architecture and file formats are not protected by copyright — the CJEU said so
directly in *SAS Institute v World Programming* (C-406/10), and *Apple Computer
v Microsoft* (35 F.3d 1435) gives user interfaces only "thin" protection,
requiring near-identity to infringe.

What you may **not** do, at any point, in any file:

- copy, vendor, transcribe or paraphrase another launcher's source, assets,
  stylesheets, locale files, icons or artwork;
- cite another project's file or line as the reason a value is what it is;
- ship another project's name, wordmark, logo or illustrations;
- take any dependency that is GPL, LGPL or AGPL (check every new crate; ISC, MIT
  and Apache-2.0 are fine, and fonts under OFL are fine).

If a design question comes up that you would normally answer by looking at the
reference's source, **answer it as a designer instead**: choose, write down why,
and make it defensible as our decision.

Two positive notes, so the prohibitions don't read as a ban on using anything:

- **Icons may come from Lucide**, whose set is ISC-licensed: use, copy, modify
  and distribute are granted for any purpose with or without fee, provided its
  copyright and permission notice travel with the files. Take them from Lucide's
  own package (`lucide-static` on npm) and carry its `LICENSE` verbatim. Do not
  take an icon set from another application's bundled copy of Lucide.
- **Fonts**: Inter at five weights is OFL 1.1 and free to redistribute as long as
  its licence text ships beside the faces. Any family you pick must satisfy the
  same test.

---

## 2. The theme: matte black, orange

Dark-first. Every value below is **ours** — a starting palette to tune, not a
measured copy of anything. Keep the roles fixed and the values editable; put them
in one file (`theme.rs`) as `const`s with a one-line reason each.

| Role | Value | Where it is used |
|---|---|---|
| `BG` | `#0B0B0C` | the window, the page pane. Matte: deliberately **not** `#000000`, so raised surfaces have somewhere to go and edges still read on OLED |
| `BG_RAISED` | `#131315` | the rail, the title bar, cards |
| `BG_INSET` | `#1A1A1D` | text fields, search boxes, the active tab's plate under a hover |
| `BORDER` | `#26262A` | hairlines, field outlines, the rule under the title bar |
| `TEXT` | `#F3F3F5` | primary text, headings |
| `TEXT_MUTED` | `#9A9AA0` | secondary text, idle icons, placeholders |
| `ACCENT` | `#FF7A1A` | the brand: primary buttons, active rail plate, focus rings, selected tabs |
| `ACCENT_HOVER` | `#FF8F3D` | hover on anything accent-coloured |
| `ACCENT_PLATE` | `#3A1F0C` | the fill behind a selected rail entry or tab |
| `ON_ACCENT` | `#0B0B0C` | the label *on* an accent fill — black, not white |
| `SUCCESS` / `WARN` / `DANGER` | your choice | states, toasts, destructive buttons |

Rules that make it a design rather than a set of colours:

- **One accent.** Orange appears on: the primary action of a surface, the
  selected navigation state, focus rings, and a progress fill. Nowhere else.
- **Matte means no pure black and no pure white.** `#0B0B0C`..`#F3F3F5`, never
  `#000`/`#FFF`, so text has headroom and the window doesn't glare.
- **Elevation is luminance, not shadow.** Raised surfaces are lighter, insets are
  darker, hairlines separate. No drop shadows except one on modal overlays.
- **Own the type ladder.** Pick a scale (e.g. 12/13/14/16/20/24), a weight per
  role, and one font family you are licensed to ship (Inter is OFL — fine).
- **Own the geometry.** Radius ladder, control heights, spacing steps: choose
  them, write them down, and use only those steps.
- **Motion: 150 ms, ease-out**, for hover fill, focus ring, switch knob, modal
  arrival. Nothing longer than 250 ms anywhere.
- **Hover brightens, press dims.** `brightness(1.1)` on hover, `0.9` on press —
  one rule, applied to the whole control, not per-part.
- Light mode is **out of scope for v2.0**. Dark only, done properly.

---

## 3. What the product must do

Features are fair game — behaviour isn't copyrightable. Build these, in this
order:

1. **Shell** — a title bar with drag regions and native window controls, a
   left icon rail, a page pane, a right panel, keyboard-navigable.
2. **Instances** — create, launch, delete. Our own on-disk descriptor (JSON),
   plus **importers** for Prism/MultiMC `instance.cfg`, vanilla `.minecraft` and
   Modrinth `.mrpack`. Format compatibility is why the importer exists; write it
   from the format's own documentation.
3. **Engine** — one pooled HTTP client, a scheduler with a global concurrency
   limit, resumable downloads, retry with backoff, a TTL'd metadata cache, a
   hash-keyed content store, Mojang piston-meta as the version source.
4. **Discover** — Modrinth search (modpacks, mods, resource packs, shaders),
   with filters, sort orders, result cards, project pages, install buttons.
5. **Settings** — a modal with the panes a launcher actually needs: appearance,
   language, java, resources, privacy, behaviour.
6. **Home** — what the app shows when nothing is running.

Not in v2.0: accounts/auth beyond offline + Microsoft device-code, skins,
screenshots, servers, a webview, telemetry.

---

## 4. Engineering rules

- **Rust stable**, `iced` for the window, `reqwest` blocking for HTTP. Every
  dependency must be justified in one line and must be permissively licensed.
- **No `unwrap`/`expect` outside tests.** Panics in a GUI are a crash with no
  message; return `Result` and show the error in the UI.
- **Comments explain why**, and the reason may never be "because another app does
  it". A number is either a written-down design decision or a measurement with
  the command that produced it.
- **Generated data must be checkable.** If you compile data (icons, a palette, a
  string table) into Rust, generate it with a tool that has a `--check` mode that
  regenerates and diffs byte-for-byte, and run it in CI.
- **Everything is committed and pushed.** A change that exists only on your
  machine has not happened. Commit in small slices; one concern per commit.
- Commit subjects: one imperative sentence, sentence case, no `feat:`/`fix:`
  prefix, body wrapped at ~80 columns explaining the *why*.

---

## 5. Verification — what "done" means

CI runs, and all must be green before a slice is finished:

```
cargo test --workspace --all-targets --locked
cargo clippy --workspace --all-targets --locked -- -D clippy::correctness
cargo fmt --all -- --check
python tools/gen_*.py --check          # every generator you add
cargo build --release --locked -p palantir-desktop   # Windows target
```

Plus the audit, which is the point of the project. A script,
`tools/licence_audit.py`, that fails when any of these is false:

| # | Assertion | How it is checked |
|---|---|---|
| 1 | No GPL/LGPL/AGPL in the dependency tree | `cargo metadata` walk over licenses |
| 2 | No GPL text anywhere in the tree | scan for `GPL`, `General Public License`, `copyleft` in source + assets |
| 3 | No file copied from another launcher | every asset and data file is listed in `THIRD_PARTY_NOTICES.md` with its origin and licence |
| 4 | No citation to another project's source | scan for `.vue`, `.scss`, `.ts:` patterns and for the reference project's name in comments |
| 5 | Every shipped asset is ours or permissively licensed | the notice file is the authority; a file not listed fails the audit |
| 6 | `[workspace.package] license` and `LICENSE` are ours | read and print them |

**The licence is the last step, not the first.** Do not change `license` in
`Cargo.toml` or write a `LICENSE` until the audit passes on a built tree. When it
does, the decision is the owner's: a proprietary notice, or MIT/Apache-2.0 if the
owner prefers open. Record the choice in `NOTES.md` with the date and the audit's
output.

**And before shipping:** the owner takes the built tree, the audit output and the
notice file to a lawyer. Nothing in this prompt substitutes for that, and design
similarity is the one question the audit cannot answer.

---

## 6. Phases

| Phase | Content | Done when |
|---|---|---|
| 0 | Empty repo, workspace, CI, the audit script, `THIRD_PARTY_NOTICES.md`, `LICENSE` placeholder | Audit fails loudly on a tree with one copied asset, passes on an empty one |
| 1 | `palantir-core`: version JSON, rules, libraries, asset index, launch arguments, data-root layout | Round-trips a real version manifest; tests against public samples |
| 2 | `palantir-net`: pooled client, scheduler, resumable downloads, metadata cache, content store | Downloads a real game version end to end with a resumed transfer |
| 3 | `palantir-loader`: vanilla, Fabric, Forge, NeoForge, Quilt installs; `.mrpack` and Prism importers | Installs each loader into a temp root and launches headless Java |
| 4 | Theme + shell: the palette above, rail, title bar, page pane, right panel | A window opens, navigates, resizes, and every colour comes from `theme.rs` |
| 5 | Instances: create, launch, kill, logs, delete | A real instance launches and exits cleanly |
| 6 | Discover: search, filters, project page, install | Search returns live results; an install lands in an instance |
| 7 | Settings modal, Home, About, error surfaces | Every pane reachable; errors say what failed and what to do |
| 8 | Audit green, own licence chosen, notices complete, first Release tagged | The Release attaches the exe, its `.sha256`, and our `LICENSE` |

Estimates, so nobody is surprised: phases 1–3 are the largest single block of
*new* work (~60–90 h), 4–7 are the visible product (~60–90 h), and 0+8 are small
but gate everything. Mark each estimate in your own progress notes as an
estimate until a measurement replaces it.

---

## 7. The one thing not to do

Do not reopen the old repository and "clean it up file by file". Exposure to a
work defeats the clean-room defence that makes this project lawful — that is
exactly the argument made against the `chardet` 7.0 rewrite in 2026, by the
original author and by the FSF, and it has not been settled in the rewriter's
favour. The new tree's safety comes from never having contained anything but our
own work, and from an audit that can prove it on demand.

Write it once, write it ours, and let the audit be the receipt.
