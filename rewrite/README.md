# PalantirMC 2.0 — the clean-room tree

A Minecraft launcher for Windows, in Rust with iced: instances, a Modrinth-backed
content browser, a launcher engine (downloads, metadata, Java) and a settings
dialog. Our own product, our own design, our own licence — written from the
specification at `docs/superpowers/specs/2026-10-08-clean-room-own-licence-rewrite.md`
in the repository root, not adapted from any existing launcher.

## The one rule

**Nothing in this tree comes from another launcher.** No code, no assets, no
interface sentences, no colour tokens, no icons, no artwork, no documentation.
What is allowed, in full:

- running a launcher as a black box and measuring what it draws;
- reading public specifications — the Minecraft version JSON and asset-index
  formats, Prism/MultiMC's `instance.cfg`, the `.mrpack` format, the Modrinth
  API docs, Mojang's piston-meta docs. File formats and functionality are not
  protected (*SAS Institute v World Programming*, C-406/10);
- icons from **Lucide's own package** (`lucide-static`, ISC), with its `LICENSE`
  travelling beside the files — never from another application's bundled copy;
- fonts we have the right to ship (Inter, OFL 1.1);
- dependencies that are permissive (ISC, MIT, Apache-2.0). Where a
  dependency offers a *choice* of licences, the permissive arm is elected
  and the election recorded in `THIRD_PARTY_NOTICES.md`; what is never
  accepted is a share-alike **obligation** -- it may exist in a dependency's
  offer, never in what this product takes.

If a design question would normally be answered by looking at a reference's
source, it is answered as a designer instead: choose, write down why in
`NOTES.md`, make it defensible as our decision.

The old tree in this repository is kept only as a working record of what came
before. It is read for nothing while writing here.

## Layout

| Path | What it is |
| --- | --- |
| `crates/palantir-theme` | the design tokens: `theme.rs`, matte black and one orange accent |
| `tools/licence_audit.py` | the receipt — six assertions, with `--selftest` |
| `THIRD_PARTY_NOTICES.md` | every file we did not write, with origin and licence |
| `NOTES.md` | phases, decisions, measurements |

## Gates

Everything must be green before a change is pushed:

```
cargo test --workspace --all-targets --locked
cargo clippy --workspace --all-targets --locked -- -D clippy::correctness
cargo fmt --all -- --check
python tools/licence_audit.py
python tools/licence_audit.py --selftest
```

`cargo fmt --all -- --check` is a hard gate here from day one. Status and the
reasoning behind each decision live in `NOTES.md`.
