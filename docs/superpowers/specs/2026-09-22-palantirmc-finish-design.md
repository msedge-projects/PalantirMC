# PalantirMC finish design

## Goal

Finish the existing PalantirMC desktop shell so its supported routes feel like
one deliberate product: every route has an intentional loading, empty,
populated, and failure state; the visual system is sourced from the copied
reference vocabulary; and the interaction layer has consistent hover, press,
focus, scrolling, and modal behavior.

This is a completion pass over the current Rust/iced application. It does not
add a new backend, replace iced, or promise feature parity with the entire
Modrinth service.

## Scope

1. Audit the existing `Page`, modal, and instance-scoped flows and complete the
   surfaces that are currently thin or placeholder-level: Home, Browse,
   Mods, Worlds, Screenshots, Logs, Settings, Accounts, About, and the
   instance/library surfaces reached from the existing state model.
2. Keep all states deterministic and local to the existing services. Network
   failures, missing instances, empty folders, and unavailable account data
   must render useful explanatory states with a recovery action where one
   already exists in the application.
3. Use the generated reference vocabulary for runtime palette, radii, and type
   values where the shell currently duplicates those values by hand. Preserve
   the generator and second-reader gates as the source of truth for future
   changes.
4. Complete the interaction pass already started in `anim.rs` and `hover.rs`:
   apply scoped card hover factors, preserve keyboard/focus-visible feedback,
   keep press feedback within iced's layout limits, and make scroll/modal
   transitions consistent across routes.
5. Add narrow regression tests for the newly completed route/state contracts,
   update the existing gate documentation, then run the locked workspace
   checks and the CI package path.

## Design constraints

- Preserve the current Rust workspace and iced 0.12 architecture.
- Do not introduce placeholder copy such as "coming soon" or blank panels for
  a route that already has a model or service behind it.
- Do not copy Modrinth trademarks or brand artwork beyond the assets already
  documented in `THIRD_PARTY_NOTICES.md`.
- Keep source comments focused on why a measured or non-obvious choice exists.
- Finish with a clean `master` commit pushed to `origin/master`; CI remains the
  authority for the release executable.

## Verification

- Route/state tests cover each supported page's non-success states and the
  existing interaction helpers cover the transition contracts.
- The task ledger under `.unlazy/palantirmc-finish/GATES.md` is linted and
  re-run after implementation.
- `cargo test --workspace --all-targets --locked` passes.
- `cargo clippy --workspace --all-targets --locked -- -D clippy::correctness`
  passes.
- `cargo build --release --locked -p palantir-desktop` passes locally as a
  fast artifact check, then the pushed CI package job is watched to completion.

## Deliberate boundary

The finish claim means the checked-in desktop shell is complete for the routes
and data sources it already exposes. It does not include Microsoft/CurseForge
service expansion, new cloud APIs, or a release tag; those remain separate
product decisions.
