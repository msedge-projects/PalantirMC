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

## Packaging the desktop

The desktop is the only deliverable this tree builds; every other crate is a
library it links. The release build is the ordinary one, from this tree's own
lock file:

```
cargo build --release --locked -p palantir-desktop
```

The binary lands at `target/release/PalantirMC.exe`. `--locked` matters more here
than anywhere else: a packaged binary has to be the tree the gates passed, not a
quiet dependency bump that only the packaging machine saw.

CI does the same build on `windows-latest` once the gates are green and uploads
the binary as a run artifact named `PalantirMC-<ref>-<run id>`. It also checks
the file exists and is a plausible size, because a build command's exit status
does not by itself prove a runnable artifact came out of it. Windows is not a
preference there: it is the only platform that compiles this tree's
`#[cfg(windows)]` code at all.

That artifact is an unsigned development build. It proves the release profile
links and produces a binary, and nothing more — there is no installer, no code
signing (which would need credentials this repository does not hold), and no
check of which DLLs the exe imports. Running it on a machine that has never had
a Rust toolchain is still the only real portability test.

Publishing is deliberately not automated. The product licence is still the
owner's `UNLICENSED` placeholder, and choosing that licence and tagging the first
Release is step 8 in `NOTES.md`: the owner's decision, taken on a built tree with
a green audit. A workflow that published on its own would be making that decision
by accident.
