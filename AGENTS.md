# AGENTS.md — how to work in this repository

Rules for any agent or tool that edits this tree. They are instructions rather
than preferences; the first one is the reason this file exists.

## 1. Push every change. Always.

**A change that exists only on this machine has not happened.** Finish every
task with a commit on `master` and `git push origin master`, without asking
first and without leaving the work uncommitted for a later session to
rediscover.

The reason is not tidiness, it is the delivery path. GitHub Actions *is* the
release build (`NEXT_STEPS.md` G4): `master` is where tests, lint, the live
service checks and both release exes come from, and a tag is what publishes
them. Nothing local is ever the artifact — `dist/` is git-ignored staging for a
file CI produced. So work that is not pushed has not been tested by the only
compiler that matters and cannot be downloaded by anyone.

What that means in practice:

- Commit as part of the change, not as a favour; one concern per commit.
- Push at the end of the task, then **confirm the run went green**:
  `gh run list --limit 3` and `gh run watch <id>`. `gh` is authenticated as
  `MSedgeMC` for this repository.
- When a run is red, fix forward with another commit. Do not amend or
  force-push a commit that is already on the remote — the runner's answer is
  the record of what failed.
- Exceptions that still need a human to ask first, because they are not
  reversible: force-pushing any branch, rewriting published history, deleting a
  remote branch, pushing a `v*` tag (which publishes a Release), or changing
  repository settings. Everything else — commit and push.

## 2. Where the exe comes from

| Trigger | Workflow | What it produces |
| --- | --- | --- |
| Push to `master` | `.github/workflows/ci.yml` | `test`, `lint`, `live`, then the `package` job builds both Windows targets and uploads `palantirmc-x86_64-pc-windows-msvc` / `-gnu` artifacts (14 days) |
| Tag `v*` | `.github/workflows/release.yml` | `guard` checks the tag against `Cargo.toml`, `build` retests and rebuilds, `publish` attaches both exes, `.sha256` sidecars and zips to a Release |
| `workflow_dispatch` | either | Re-run without a new commit; `release.yml` needs an existing tag |

To get a fresh build locally, do not compile one by hand and call it current:
push, then

```
gh run download <run-id> -n palantirmc-x86_64-pc-windows-msvc -D dist/
```

CI's artifact is the authority; a local `cargo build` is a convenience that has
already been wrong once about what the runner accepts.

## 3. Commands that must pass before the push

```
cargo test --workspace --all-targets --locked
cargo clippy --workspace --all-targets --locked -- -D clippy::correctness
cargo build --release --locked -p palantir-desktop      # only if the exe matters
```

`--locked` is deliberate everywhere: it is how a manifest and `Cargo.lock` that
have drifted are caught before the runner catches them.

These are the commands **CI** runs, and CI is where they have to pass. On a
machine where a full workspace run does not finish inside a working session, the
loop that works is the targeted one -- `cargo test -p <the crate you changed>
--locked` while you iterate -- and then push and `gh run watch` the run that does
all of it. A local workspace run is a convenience; the runner's answer is the
gate, which is §1's rule read from the other end. `package` depends on
`test`, so a compile or test failure is cheap to fix and the release build is
what a late failure costs.

The live tests are `#[ignore]`d by design and run with
`cargo test -p palantir-net --test live --locked -- --ignored --test-threads=1`.
A failure there is a failure, not a network hiccup.

## 4. Commit messages

The subject is one imperative sentence naming what the change does or why,
sentence case, no `feat:`/`fix:` prefix, no trailing period. The body is prose
that explains the reason it is this way — what was measured, what was wrong
before, what was deliberately not done — wrapped at ~80 columns. Real subjects
from this history:

```
Read the metadata lists in the order they arrive
Ask the metadata which Fabric build is current instead of naming one
Keep this launcher's data in its own folder, and offer the install that is already there
```

Commits made by an agent end with the trailer:

```
🤖 Generated with Codebuff
Co-Authored-By: Codebuff <noreply@codebuff.com>
```

## 5. Conventions the code already holds

- **No `unwrap`/`expect` outside tests.** Several crates deny them crate-wide;
  a new crate that does the same needs the `cfg_attr(test, allow(...))` line
  *before* the `deny`, because inner attributes apply in sequence.
- **Comments explain why**, especially where a decision looks odd: the reason a
  value was chosen, or the failure that made it necessary. `NEXT_STEPS.md` and
  `GATES.md` are where the long-form reasoning lives, with the measurements.
- **Never commit build output or captures.** `dist/`, `target/`, `*.log` and
  `.scratch/` are ignored; keep them that way.
- **Adopted third-party code keeps its own formatting and lints**, and every
  edit to it is recorded in `THIRD_PARTY_NOTICES.md`. An adopted crate is
  merged with upstream later, not re-derived.
- **Attribution is a shipping condition**, not a nicety: art taken from another
  launcher and code taken under MIT both carry their notice in the source and
  in the binary's About page.
