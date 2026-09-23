# PalantirMC

A Minecraft launcher for Windows, written in Rust with [iced](https://iced.rs).
Its shell is ported element-by-element from the Modrinth App's own running
window — every colour, length, weight and radius in `crates/palantir-desktop`
is a measured value, and every claim about it is checked by a gate that the
build it replaced fails.

**Read `AGENTS.md` before changing anything.** It is the working contract for
this tree, not a suggestion file: every change is committed to `master` and
pushed, GitHub Actions *is* the release build, and the commands that must pass
before a push are listed there.

## The four documents that run this project

| Document | What it is |
| --- | --- |
| `AGENTS.md` | How to work here: commit/push rules, the commands CI runs, conventions the code already holds. Start here. |
| `NEXT_STEPS.md` | The project's memory: 30 numbered sections of deferred work, measurements, decisions made and decisions deliberately not taken. When a choice looks odd in the code, the reasoning is in here. |
| `GATES.md` | The recorded gate results: what each gate asserts, the command that runs it, and the capture it was judged from (G1–G57). |
| `REFERENCE.md` | The design reference: what was measured off the Modrinth App, token by token, and how the shell draws it. |

A new contributor — human or AI — should read `AGENTS.md` fully, then skim
`NEXT_STEPS.md`'s section headings and `GATES.md`'s gate list before writing
code.

## Layout

```
crates/
  palantir-desktop/   The Windows launcher itself (iced): shell, pages, theme,
                      animation, the gate-checked design port. This is where
                      almost all current work happens.
  palantir-core/      Shared domain types and product constants.
  palantir-net/       Network: Modrinth/CurseForge API clients, downloads.
  palantir-loader/    Instance loading and launch pipeline.
  palantir-gui/       Headless GUI model logic shared by the frontends.
  palantir-cli/       Command-line interface and round-trip checks.
  nbt/, schema/       Adopted wholesale from PandoraLauncher (MIT, Copyright
                      (c) 2025 Moulberry). Kept byte-identical to upstream so
                      they can be merged with it later — do not restyle them;
                      every edit is recorded in THIRD_PARTY_NOTICES.md.
tools/               The measurement and gate harnesses (Python).
vendor/modrinth-app/ The reference client's source, vendored as the design
                     oracle. It is read and measured, never built or shipped.
docs/superpowers/    Working specs (dated), e.g. the finish-design pass.
licenses/            Third-party licence texts.
dist/, target/, .scratch/, *.log   Build output, CI artifacts, captures —
                     git-ignored, never committed.
```

## Building and checking

Rust (stable) and Python 3 are the toolchain; the launcher targets Windows.

```
cargo test -p palantir-desktop --locked        # the crate you changed
cargo test --workspace --all-targets --locked  # what CI runs
cargo clippy --workspace --all-targets --locked -- -D clippy::correctness
cargo build --release --locked -p palantir-desktop   # -> target/release/PalantirMC.exe
```

`--locked` is deliberate everywhere: a drifted manifest must fail here, not on
the runner.

**Where the exe comes from:** a push to `master` runs `.github/workflows/ci.yml`
(test → lint → live services → both Windows release builds), and the run's
artifacts are the authority. Do not compile locally and call that current —
download the artifact:

```
gh run download <run-id> -n palantirmc-x86_64-pc-windows-msvc -D dist/
```

A tag `v*` publishes a Release (`.github/workflows/release.yml`); pushing such
a tag is one of the few actions that needs a human's go-ahead first (see
`AGENTS.md` §1).

## The gates

The design is not "close to the reference", it is *asserted*: each gate is a
command comparing a capture of this launcher against numbers measured off the
Modrinth App, and each was written so the build it replaced fails it.

- `tools/panel_gate.py` — the shell chrome (title bar, rail, gutter).
- `tools/page_gate.py` — page content, element by element.
- `tools/appshot.py` — captures this launcher's own frame unattended
  (`--page home`, `--page discover`, …) at an exact client size; the page
  gates run against its output.
- `tools/gen_tokens.py` — regenerates `crates/palantir-desktop/src/
  theme_tokens.rs` from the reference's stylesheets; nothing in that file is
  hand-edited, and the regeneration must be byte-identical.

Captures live in `.scratch/` and are never committed. `GATES.md` records which
gates are environment-dependent and what a green run means for each.

## Conventions the code already holds

- No `unwrap`/`expect` outside tests; several crates deny them crate-wide.
- Comments explain *why*, especially where a decision looks odd; long-form
  reasoning goes into `NEXT_STEPS.md` and `GATES.md`.
- Commit subjects are one imperative sentence, sentence case, no prefix.
- Adopted third-party code keeps its own formatting and is merged with
  upstream later, not re-derived.
- Attribution is a shipping condition: third-party art and code carry their
  notice in source and in the binary's About page.

## Licence and attribution

PalantirMC is `GPL-3.0-only` (`Cargo.toml`). It incorporates code from
PandoraLauncher (MIT, Copyright (c) 2025 Moulberry) under
`licenses/PandoraLauncher-LICENSE.txt`, and vendors the Modrinth App source
(GPL-3.0, pinned in `vendor/modrinth-app/UPSTREAM.md`) as a read-only
measurement reference — it is never compiled or shipped. Full notices:
`THIRD_PARTY_NOTICES.md`.
