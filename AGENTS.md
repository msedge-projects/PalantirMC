# AGENTS.md — how to work in this repository

Rules for any agent or tool that edits this tree. They are instructions rather
than preferences; the first one is the reason this file exists.

## 1. Push every change. Always.

**A change that exists only on this machine has not happened.** Finish every
task with a commit, pushed to both remotes, without asking first and without
leaving the work uncommitted for a later session to rediscover:

```
git push origin <branch>   # MSedgeMC/PalantirMC, private, the working record
git push public <branch>   # msedge-projects/PalantirMC, public, where CI runs
```

The reason is not tidiness, it is the delivery path. GitHub Actions *is* the
release build (`NEXT_STEPS.md` G4): `master` is where tests, lint, the live
service checks and both release exes come from, and a tag is what publishes
them. Nothing local is ever the artifact — `dist/` is git-ignored staging for a
file CI produced. So work that is not pushed has not been tested by the only
compiler that matters and cannot be downloaded by anyone.

**Why there are two remotes.** The account behind `origin` ran out of Actions
minutes: every push to it from G101 onwards produced a zero-step run reading
`recent account payments have failed or your spending limit needs to be
increased`, and the push that carried G112 produced no run at all. Actions
minutes are free for public repositories on standard runners, and the `package`
job is Windows-only, billed at twice a Linux minute wherever minutes are
charged. So the tree is mirrored to `public`, where the same workflows run for
free and the artifacts come from. The mirror is one-way in practice: it is
never edited, never merged, and the working record stays on `origin`.

**A push schedules nothing without an open pull request.** `ci.yml` narrows
`push` to `master` and carries every other branch through `pull_request`, so
the mirror holds a draft PR (`msedge-projects/PalantirMC#1`) from the working
branch into its own `master`: each push re-runs CI under that PR. The private
repository's PR (`MSedgeMC/PalantirMC#10`) still exists and still collects runs
that cannot start.

What that means in practice:

- Commit as part of the change, not as a favour; one concern per commit.
- Push at the end of the task, then **confirm the run went green**:
  `gh run list --repo msedge-projects/PalantirMC --limit 3` and
  `gh run watch <id> --repo msedge-projects/PalantirMC`.
- The two accounts are held by per-remote credential helpers in `.git/config`
  (`gh auth token -u <account>`), so pushing needs no account switching and the
  active `gh` account does not matter for a push. It does matter for `gh api`
  calls against a private repository: `gh auth switch -u MSedgeMC` first.
- `AGENTS.md`'s rules apply to both remotes. In particular a `v*` tag publishes
  a Release wherever it is pushed, and on the mirror that Release is public.
- When a run is red, fix forward with another commit. Do not amend or
  force-push a commit that is already on the remote — the runner's answer is
  the record of what failed.
- Exceptions that still need a human to ask first, because they are not
  reversible: force-pushing any branch, rewriting published history, deleting a
  remote branch, pushing a `v*` tag (which publishes a Release), or changing
  repository settings. Everything else — commit and push.

## 2. Where the exe comes from

Every row below describes the **public mirror**, because that is the only
remote whose runs start. Name it with `--repo msedge-projects/PalantirMC` on
every `gh run` and `gh workflow` command; the private remote answers but its
jobs never leave the queue.

| Trigger | Workflow | What it produces |
| --- | --- | --- |
| Push to the mirror's PR branch | `.github/workflows/ci.yml` | `test`, `lint`, `live`, then the `package` job builds both Windows targets and uploads `palantirmc-x86_64-pc-windows-msvc` / `-gnu` artifacts (14 days) |
| Push to the mirror's `master` | `.github/workflows/ci.yml` | the same list, without needing a PR — but no `ci.yml` run has ever carried `master`, so this row describes an intention rather than a thing that has happened; see below |
| Tag `v*` | `.github/workflows/release.yml` | `guard` checks the tag against `Cargo.toml`, `build` retests and rebuilds, `publish` attaches both exes, `.sha256` sidecars, zips and `LICENSE` to a **public** Release |
| `workflow_dispatch` | either | Re-run without a new commit; `release.yml` needs an existing tag |

The `master` row is the one with no receipt, and that is now a measurement rather
than a silence: `gh run list --branch master` returns exactly two runs,
`36574518355` and `36574524609`, and both are **Dependabot Updates** rather than
this repository's `ci.yml`. Filtering a run list by branch cannot tell the two
apart — read the workflow name, or a green `master` will look like this
workflow having run there. The silence that first made the row doubtful is not
explained by the `paths-ignore` above either: a filter gives up and the workflow
runs once a push is bigger than 300 files, and the creation push carried the
whole tree, so it should have started one. The row stays open.

Do not close it by fast-forwarding `master` to the working branch. That push puts
the pull request's own head commits into its base, and GitHub reads commits in
the base as the change having landed and closes the pull request for it — and
that request is the only reason a push to any other branch starts anything here,
because `push` is narrowed to `master`. The mirror would then be as silent as the
private remote, with the last slices still to land. To run the jobs against the
master tree without moving a pointer, ask for them instead:

```
gh workflow run ci.yml --repo msedge-projects/PalantirMC --ref master
```

To get a fresh build locally, do not compile one by hand and call it current:
push, then

```
gh run download <run-id> --repo msedge-projects/PalantirMC \
  -n palantirmc-x86_64-pc-windows-msvc -D dist/msvc
gh run download <run-id> --repo msedge-projects/PalantirMC \
  -n palantirmc-x86_64-pc-windows-gnu  -D dist/gnu
```

Both artifacts contain a file called `PalantirMC.exe`, so one directory cannot
hold both: the second download fails with `error extracting "PalantirMC.exe":
The file exists` and leaves the first target's exe in place, which reads as a
successful download of the wrong binary. The `.sha256` beside each one is what
tells them apart.

Each artifact is staged by the `package` job only after
`.github/scripts/check_exe.py` has read the import table of that exact file, and
that gate is why the MSVC target is compiled with `+crt-static`: a build that
imports `VCRUNTIME140.dll` does not start on a clean Windows. The artifact is the
expected binary, not just a build of it.

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

## 6. Never idle through a long command

The session is billed by wall-clock time, not by tokens, so the most expensive
thing an agent can do in this tree is wait. A full workspace run is minutes of
one core; watching it pass, or watching it fail slowly, spends those minutes on
nothing at all. There is no saving in the wait.

So when a command is expected to take longer than about a minute, start it and
start something else in the same breath — write the next slice, draft the doc
paragraph, review the diff just written, read the code the next change touches.
Concretely:

- **Start it detached, check it at a boundary.**
  `nohup <cmd> > .scratch/<name>.log 2>&1 & echo $? > .scratch/<name>.exit`,
  then poll the `.exit` file after a real piece of work lands instead of
  blocking the turn on it.
- **Let the runner do the heavy compiling.** The full workspace test, the
  clippy sweep and the release build are what the mirror exists for (§2); a
  targeted `cargo test -p <the crate you changed> --locked` is the local
  iteration loop. Two agents share one machine and contend for the same cores,
  while a pushed run compiles beside both of them — so push earlier than feels
  natural and keep working through it.
- **`gh run watch` is a last step, not a way to pass time.** Poll
  `gh run list --repo msedge-projects/PalantirMC --limit 3` at slice
  boundaries, and only block once there is genuinely nothing left that does not
  depend on the answer.
- **Never run a heavy build and a heavy build at once on this machine.** A
  second `cargo` invocation on the same target directory waits on the first
  one's file locks and burns both sessions; queue it or push it instead.

This is the same rule as §1 read from the machine's side: the runner is the
only compiler that matters and it costs no local clock. Local CPU is for the
narrow check that tells you whether to push yet.

## 7. Report progress as you go

The user watches a live transcript and has asked for the state of the work
without having to ask for it. Report at these points, in two or three lines:

- **At every slice boundary** — a commit, a pushed run, a gate added: what
  landed, what the checks said in numbers, and what is next by the plan's own
  name.
- **At every parallel switch** — before starting each part of a multi-part
  task, and again when it finishes: what is running, what is being written
  meanwhile.
- **At least every ~15 minutes of wall-clock work** even when nothing has
  landed: what is compiling or testing, what is being read or written in
  parallel, and whether the estimate moved.

Keep it quantitative and terse: stages and slices from `NEXT_STEPS.md`, the
test counts from the run in hand, and a rough percentage against the stage
table -- with a one-line note when a number is an estimate rather than a
measurement. Update `NEXT_STEPS.md`'s stage table in the same commit as the
slice that moved it, and take the percentage from `python tools/progress.py`
(or, where no working `python` is on PATH, `tools/progress.cmd` -- it finds an
interpreter and reads the same documents) rather than writing one by hand: it
reads that table and `GATES.md`'s ledger, so the two cannot drift apart, and it
fails when they do.
