# Modrinth App, native in Rust

Status: active. Supersedes `2026-09-22-palantirmc-finish-design.md`, which was a
completion pass over a shell this replaces.

## Goal

The launcher behaves and reads as the **Modrinth App**, implemented natively in
Rust with iced — no webview, no Tauri, no Node in the build. Modrinth's own source,
vendored read-only at `vendor/modrinth-app/` and pinned in its `UPSTREAM.md`, is
the only design reference. Prism Launcher stops being a reference for anything:
not its formats, not its meta API, not its navigation, not its icons.

## Why the previous shell is being replaced

Recorded in full in `REFERENCE.md` and `GATES.md`; briefly, three causes:

1. **The information architecture was deliberately not copied.** The port matched
   the reference's geometry while keeping a flat rail with Mods, Worlds, Logs,
   Settings, Accounts and About as top-level pages. The reference keeps those
   inside an instance (`/instance/:id/...`) and opens Settings as a modal from the
   rail. `REFERENCE.md`'s "Still to port" says so in as many words.
2. **The design system was transcribed, not wired in.** 189 tokens went into a
   test-only module while the shell painted from a hand-written palette; the
   reference declares 362 custom properties.
3. **Motion and icons were never ported.** 193 vendored files declare transitions;
   the reference's hover/press was recorded as "as before". Icons were carved from
   another launcher's binary rather than taken from the reference's 315 SVGs.

## Decisions

| Question | Decision |
| --- | --- |
| Frontend | Native Rust, 1:1 with the reference's style, motion, fonts and effects. No webview. |
| Backend | Lightest. Our crate graph stays; no `theseus`, no SQLite, no tokio. The engine around the existing blocking HTTP client is what gets fixed. |
| Instances | Our own descriptor, plus importers for the popular launchers. The importer is also the migration path, so there is one code path and not two. |
| Gates | Rebuilt and kept. They are the only reason "1:1" can be a command's verdict instead of a claim. |

## Stages

| Stage | Content |
| --- | --- |
| 0 | **Done.** Prune what nothing references (`crates/nbt`, `crates/schema`, `crates/palantir-cli`, their notices, the superseded spec, local staging) and reorganize the documents (`NOTES.md` lifted out of `NEXT_STEPS.md`). |
| 1 | **Done.** The generated design system. `tools/gen_theme.py` compiles the reference's CSS custom properties, Tailwind's default theme and the component transition blocks into a `theme_gen.rs` the shell paints from, plus a motion table; `tools/gen_icons.py` turns the vendored SVGs into an icon set; `tools/gen_text.py` compiles both English locales into the string table. Regeneration of all three must be byte-identical. |
| 2 | **Done.** The shell, on the reference's IA. Rail, head, page pane, right panel, status bar; a `Route` tree with children for instance and project nesting; Settings as a modal; effects and the tween engine driven by the motion table; `color_theme.rs` for the setting; `tests/native.rs` for the no-webview premise. |
| 3 | **In progress.** Pages, in the reference's order: instance pages first (Content, Files, Worlds, Screenshots, Logs, Share), then Project, Home, Discover's six tabs, Skins, Screenshots, Servers, User — each with its own gate and its empty/loading/error states. All eight modules are in and the pane draws them; Discover's search is answered by the engine (G75), and a page that needs a service says what is missing rather than drawing an empty list. The remaining gaps inside this stage are recorded in `NEXT_STEPS.md`: the pages' controls do not yet tween their hover off the same clock the rail's plate uses, the right panel is still the reference's wash rather than a service's answer, and Settings is still a placeholder modal. |
| 4 | **In progress.** The backend engine: one pooled client, a scheduler with a global concurrency limit, resumable and cancellable downloads, retry with backoff, one TTL'd metadata store, a hash-keyed content store, Mojang piston meta plus the Modrinth API as the metadata source. The transport spine, the content store, both metadata layers and the seam that lets a page use them are in and gated (G66-G75 in `GATES.md`); what remains is the desktop's own call sites -- `browse.rs` and `launch.rs` still hold clients of their own -- which moves with the pages that need the scheduler and the content store beside them. |
| 5 | **Instances in our own format**, with importers: Prism/MultiMC, Modrinth App, vanilla `.minecraft` and CurseForge first, then GDLauncher, ATLauncher and Technic. |

## Constraints

- No new Rust dependency for anything the reference's own stack would not need:
  the design system and the icon set are generated offline by Python tooling, and
  the backend keeps `reqwest`'s blocking client.
- The reference tree is never compiled or shipped. Its art stays reference: sizes,
  boxes, colours, slots. The launcher draws its own mark, and Modrinth's marks stay
  theirs (GPL-3.0 covers the code, not the identity).
- Adopted third-party code keeps its own formatting and is merged with upstream
  later rather than re-derived; every edit lands in `THIRD_PARTY_NOTICES.md` first.
- Source comments explain *why*. A value taken from the reference is generated or
  cited, never asserted.

## Verification

- Regeneration of the design system is byte-identical, and its gate asserts every
  property it claims in every theme.
- Each page gate is written so the build it replaces fails it.
- `cargo test --workspace --all-targets --locked` and clippy with
  `-D clippy::correctness` pass, and the pushed run is watched to green — CI is
  the authority, per `AGENTS.md`.
- **When the runner cannot start, the stage is not done.** Stages 2 and 3 were
  pushed while GitHub Actions was refusing to schedule jobs on this account
  (runs `36038333030` and `36033443993`: five seconds, zero steps, "recent
  account payments have failed"). The three commands above were run locally with
  the same flags and their transcripts are in `GATES.md`, and `NEXT_STEPS.md`
  records that this is a weaker receipt than a green run: a local run is not a
  clean checkout. The push is re-run with `workflow_dispatch` when billing is
  restored, and nothing from that window is described as "CI passed".

## Deliberate boundary

This spec does not promise pixel-identical rendering to a browser: it promises
shared tokens, shared geometry, shared motion numbers and ink-box-level text
equality, measured off the reference's own window. `backdrop-filter` and CSS
transforms are the two places a native toolkit cannot be exact, and each is
handled explicitly rather than approximated silently.
