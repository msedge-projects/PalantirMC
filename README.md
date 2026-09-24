# PalantirMC

A Minecraft launcher for Windows by **Palantir Studios**, written in Rust with
[iced](https://iced.rs). It is a **native implementation of the Modrinth App**:
its design system, information architecture, motion and icons are ported from the
Modrinth App's own source, vendored read-only at `vendor/modrinth-app/`, and its
backend is our own Rust.

The design is not "close to the reference", it is *asserted*: every colour,
length, weight and radius in `crates/palantir-desktop` is a measured value, and
each claim about it is checked by a command that the build it replaced fails
(`GATES.md`).

**Read `AGENTS.md` before changing anything.** It is the working contract for
this tree, not a suggestion file: every change is committed and pushed, GitHub
Actions *is* the release build, and the commands that must pass before a push are
listed there.

## The documents that run this project

| Document | What it is |
| --- | --- |
| `AGENTS.md` | How to work here: commit/push rules, the commands CI runs, conventions the code already holds. Start here. |
| `NEXT_STEPS.md` | Where the project stands and what is being done to it: the stage table, and where the old sections went. |
| `NOTES.md` | The engineering record — what was measured and what it cost. Sections keep their old `NEXT_STEPS.md` numbers because source comments cite them. |
| `GATES.md` | The recorded gate results: what each gate asserts, the command that runs it, and the capture it was judged from. |
| `REFERENCE.md` | The design reference: what was measured off the Modrinth App, token by token, and how the shell draws it. |
| `docs/superpowers/specs/` | Dated working specs. The active one is `2026-09-24-modrinth-native-rewrite.md`. |

## Layout

```
crates/
  palantir-desktop/   The Windows launcher itself (iced): shell, pages,
                      platform code and the engine glue. This is where almost
                      all current work happens.
  palantir-core/      Minecraft's own formats — version JSON, libraries, rules,
                      asset index, launch arguments — and the data-root layout.
  palantir-net/       Auth, downloads, metadata and the Modrinth API client.
  palantir-loader/    Forge, Fabric, NeoForge, Quilt and modpack archives.
  palantir-gui/       The view-model the shell reads; goes with `app.rs`.
tools/               The measurement and gate harnesses (Python).
vendor/modrinth-app/ The reference client's source, vendored as the design
                     oracle and pinned in its `UPSTREAM.md`. Read and measured,
                     never built or shipped.
docs/superpowers/    Working specs (dated).
target/, dist/, .scratch/, *.log   Build output, CI artifacts, captures —
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
the runner. On a machine where the full workspace run does not fit in a session,
iterate with `cargo test -p <the crate you changed> --locked` and let the runner
run all of it — and do not pipe a long `cargo` command into `tail`, because the
progress lines are buffered and a working build looks like a hung one.

**Where the exe comes from:** a push to `master` runs `.github/workflows/ci.yml`
(test → lint → live services → both Windows release builds), and the run's
artifacts are the authority. Do not compile locally and call that current —
download the artifact:

```
gh run download <run-id> -n palantirmc-x86_64-pc-windows-msvc -D dist/
```

A tag `v*` publishes a Release (`.github/workflows/release.yml`); pushing such a
tag is one of the few actions that needs a human's go-ahead first (see
`AGENTS.md` §1).

## The gates

- `tools/panel_gate.py` — the shell chrome (title bar, rail, gutter).
- `tools/page_gate.py` — page content, element by element.
- `tools/shellcmp.py` — two windows measured against *each other*, for the class
  of mistake neither single-window gate can see: a part drawn to a plausible
  number in both clients but offset in one.
- `tools/appshot.py` — captures this launcher's own frame unattended
  (`--page home`, `--page discover`, …) at an exact client size; the page gates
  run against its output. The reference's own captures come from `winshot.py`,
  driven by `refwalk.py`.
- `tools/gen_theme.py` — compiles the design system out of the reference's own
  stylesheets into `theme_gen.rs`. Nothing it emits is hand-edited, and
  `--check` regenerates and compares byte for byte.
- `tools/gen_tokens.py` — the retiring half of the same job: it regenerates
  `theme_tokens.rs`, the test-only receipt for 189 values that the shell paints
  beside rather than from. It stays until the last page paints from `theme_gen`,
  because until then it is the only token gate the tree has.
- `tools/unused_deps.py` — the unused-dependency check `cargo` does not have: a
  manifest that declares what no source in its crate names. A hit is a question
  to read, not a verdict (a package's lib name is not always its package name).
  Not wired into CI: `tools/**` deliberately starts no run.

Captures live in `.scratch/` and are never committed. `GATES.md` records which
gates are environment-dependent and what a green run means for each.

## Conventions the code already holds

- No `unwrap`/`expect` outside tests; several crates deny them crate-wide.
- Comments explain *why*, especially where a decision looks odd; long-form
  reasoning goes into `NOTES.md`, `GATES.md` and the specs.
- Commit subjects are one imperative sentence, sentence case, no prefix.
- Adopted third-party code keeps its own formatting and is merged with upstream
  later, not re-derived.
- Attribution is a shipping condition: third-party art and code carry their
  notice in source and in the binary's About page.

## Licence and attribution

PalantirMC is `GPL-3.0-only` (`Cargo.toml`), © Palantir Studios. It vendors the
Modrinth App source (GPL-3.0, pinned in `vendor/modrinth-app/UPSTREAM.md`) as a
read-only measurement reference — never compiled, never shipped. Full notices:
`THIRD_PARTY_NOTICES.md`. Modrinth's name, wordmark and logo are its marks, and
GPL-3.0 grants rights in the code and not in the identity: this launcher draws
its own mark in the same slots.
