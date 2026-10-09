# AGENTS.md — working context for the clean-room tree

Everything an agent needs before touching `rewrite/`, kept here so a
session restart does not lose it. Repository-wide rules live in
`../AGENTS.md` and win on conflict; engineering *decisions* live in
`NOTES.md`. This file holds the standing instructions, the exact gates,
and where the work stands.

## 1. Standing instructions (owner, 2026-10-09)

- **PROTECTED: Phase 4 (theme + shell) must not be started here —
  and neither may any UI code.** The owner will run the UI work with
  a different tool, under an explicit instruction that names it.
  Reaching for phase 4 because it is "next" is precisely the accident
  this rule exists to prevent: no scaffolding, sketches, stubs, or
  generated UI files from any agent in this tree. This covers the
  whole UI block (4 theme + shell, 5 instances, 6 Discover,
  7 settings/Home/About). They stay `not started` until the owner
  assigns them; treat touching them as a mistake to stop and report,
  not a step to take.
- **Never wait idle on a compile, test run, or CI.** While one runs in
  the background, do the next thing: write the next module, analyze the
  next format, fetch the next fixture, review the last diff. A polling
  loop that does nothing else is waiting; polling interleaved with real
  work is progress.
- **Test a feature as it is written.** The moment code exists, kick its
  tests off in the background and keep building; collect and act on the
  result before claiming anything is done. A finished feature is one
  whose tests have *run*, not one whose tests exist.
- **Background jobs go through the background runner**, each with its
  own log file (bare `&` and detached shells do not survive tool
  cleanup). Exit codes travel in the log — read that log before
  reporting success.

## 2. Gates before every push (run from `rewrite/`)

```
cargo fmt --all -- --check
cargo check --workspace                       # regenerates Cargo.lock first
cargo clippy --workspace --all-targets --locked
cargo test --workspace --locked
python3 tools/licence_audit.py                # expect 6/6
python3 tools/licence_audit.py --selftest     # flag is --selftest
```

- Clippy must be at **zero warnings**; the check is
  `grep -cE "^warning: [a-z]|^error"` over the clippy output and the
  count must be 0.
- `--locked` fails after adding a dependency until `cargo check
  --workspace` has regenerated `Cargo.lock` — run it first, not last.
- When filtering command output through a pipeline, preserve the exit
  code explicitly (`set -o pipefail`, or capture `${PIPESTATUS[0]}`);
  a filtered pipeline must never hide a failure.
- Windows-only failures are real failures: separator-joined path
  *strings*, symlinks, shared temp dirs. Design tests platform-neutral
  — compare paths as `Path`, not as strings — and take the fix in the
  production code where the portability bug actually lives.

## 3. Commits and push

- One concern per commit. Subject: one imperative sentence, sentence
  case, no prefix, no trailing period; body prose wrapped ~80 columns
  explaining *why*. Trailer on every agent commit:

  ```
  🤖 Generated with Codebuff
  Co-Authored-By: Codebuff <noreply@codebuff.com>
  ```

- **Push both remotes** — `origin` (private record) and `public`
  (mirror where CI runs):

  ```
  git push origin master
  git push public master
  ```

- Confirm CI with `gh run watch <id> --repo msedge-projects/PalantirMC
  --exit-status`, kicked to a background job with its own log while
  other work continues. The legacy `ci.yml` runs too (~13–15 min,
  5 jobs); `rewrite.yml` is the one to watch first (Audit+lint+test,
  plus a Live-services job on a Windows runner).

## 4. Facts worth not re-deriving

- **Java runtime index pin**: the path segment is a content hash and
  goes stale — `https://piston-meta.mojang.com/v1/products/java-runtime/2ec0cc96c44e5a76b9c8b7c39df7210883d12871/all.json`.
  A 404 means "update the pin", never "no runtimes". Manifests map
  `file` (raw+lzma, `executable`), `directory`, `link` (`target`).
  Mojang platform names: `windows-x64`, `windows-x86`, `linux`,
  `linux-i386`, `mac-os`, `mac-os-arm64` (no linux-aarch64).
- **Fabric meta**: `https://meta.fabricmc.net/v2/versions/loader/{game}`
  (entries wrap `loader: {version, stable}`), profile at
  `/v2/versions/loader/{game}/{loader}/profile/json`. **Quilt**:
  `https://meta.quiltmc.org/v3/versions/loader/{game}` plus the
  matching `/profile/json`; Quilt sets no stable flags and lists
  newest-first. Pick = newest stable, else newest outright.
- **Forge/NeoForge installers** carry `install_profile.json` (`spec: 1`)
  plus a `version.json` overlay. Token forms: `[coord]` → Maven path
  under the library root; `{MARKER}` → the side's `data` value
  (bracketed path, `'quoted literal'`, or plain literal — BINPATCH is
  plain, MCP_VERSION quoted); built-ins `{ROOT}`, `{INSTALLER}`,
  `{MINECRAFT_JAR}`, `{SIDE}`. Unknown tokens error *naming the token*.
  `outputs` are skip receipts. Planning lives in `installer.rs`; the
  run is the launch slice.
- **`Version::merged_with(parent)`**: child wins scalars; library
  identity is `group:artifact[:classifier][@ext]` without version; child
  libraries lead the classpath; arguments append parent-first;
  minima merge to max; the result clears `inheritsFrom`.
  `Arguments` lists are `Option<Vec<Argument>>` — absent is not
  present-empty.
- **`palantir-net` surface**: `Http::new()`, `download::download`,
  `sha1_bytes`/`sha1_file`, `Scheduler::new(n)`, `ContentStore`,
  `Syncer::sync_version(version, platform, game_dir) -> (SyncReport,
  Option<Transfer>)`, `MetadataCache` with `fetch_text(http, key, url,
  ttl)`.
- **`test-support`**: the shared mock HTTP server (`set_route`, `hits()`,
  `range_offsets()`, one-shot `truncate_after`), a dev-dependency of
  every crate that tests against a wire.
- **Fixtures** are real public documents and every one needs a row in
  `THIRD_PARTY_NOTICES.md` (audit check #5 fails on unlisted files).
- No `unwrap`/`expect` outside tests; comments explain *why*; permissive
  dependencies only (MIT/Apache/ISC).

## 5. Work state — update at every slice

Phases 0–2 landed and CI-green (83+ offline tests, 2 ignored live
tests: real 1.5.2 end-to-end sync, real resumed jar).

**Phase 3 (`palantir-loader`) — in progress.** Slice map, in
`lib.rs` too:

- [x] `Version::merged_with` + merge fixtures + tests
- [x] crate skeleton, `test-support` extraction
- [x] `install.rs` — resolve chain + `install_document`
- [x] `java.rs` — runtime index/manifests, fetch, platform names
- [x] `profiles.rs` — Fabric/Quilt/Forge-family listing + `install_loader`
- [x] `installer.rs` — install-profile parse + processor planning
- [x] `import.rs` + `mrpack.rs` + `prism.rs` + `curseforge.rs` +
      `vanilla.rs` — the importers. One order form (`ImportedPack`),
      one translation per packaging: Modrinth `.mrpack`, Prism/MultiMC
      (`instance.cfg` + `mmc-pack.json`), CurseForge `manifest.json`
      (CurseForge app, GDLauncher, ATLauncher exports), and the vanilla
      `.minecraft` layout (official, TLauncher, SKLauncher, Badlion,
      Legacy, Lunar, Feather). Unknown loader ids become
      `LoaderTarget::Unknown`, never vanilla.
- [x] `launch` — natives extraction (`extract_natives`, honours the
      `extract` excludes) and `build_launch_plan`: classpath resolved
      from the version's own libraries for the platform, every
      placeholder the format defines expanded (no `${...}` survives
      into the command), pre-2018 `minecraftArguments` split, loader
      overlays planned after `merged_with`. `LaunchPlan::command`
      hands back a spawnable `std::process::Command` (supplies `-cp`
      for pre-2018 versions that name no JVM list). 6 plan tests
      against the real fixtures.
- [x] processor runner — `run_processors` in `launch.rs` spawns the
      planned Forge processors as headless Java (jar + classpath on
      `-cp`, `Main-Class` from the jar's manifest with continuation
      lines joined), honours the install profile's skip receipts
      (outputs already at their promised hashes never run), and treats
      a run that exits badly or breaks its promises as `Error::Processor`
      naming the jar. 7 offline tests with a cross-platform mock Java
      (CI runs windows-latest).

**Done-when for Phase 3**: each loader (vanilla, Fabric, Forge,
NeoForge, Quilt) installs into a temp root and launches headless Java.
Likely an `#[ignore]` live test, like `palantir-net`'s.

Importers translate to "game version + loader + files" and install
through the existing door (`install_document` / `install_loader`), with
offline tests against real fixtures. After Phase 3: phase 4 (theme +
shell — **protected, see §1: no UI work until the owner assigns it to
another tool explicitly**), 5 (instances), 6 (Discover),
7 (settings/Home/About), 8 (audit green, own licence, first Release).
