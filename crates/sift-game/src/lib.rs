//! # sift-game
//!
//! The launcher's game engine: the half that resolves a version, installs what
//! is missing, picks a Java and starts Minecraft. It exists so that the engine is
//! not owned by one UI: `sift-desktop` (iced) and `crates/sift-ui` (egui, its own
//! workspace) are two shells over the same code, and the extraction is what lets
//! the first be deleted without losing the second's ability to install and run a
//! game.
//!
//! ## What moved
//!
//! `sift-desktop/src/launch.rs` was moved whole -- functions, documentation,
//! "why" comments and tests -- together with the modules it could not compile
//! without, because a launch is not one file:
//!
//! * [`launch`] -- the launch plan, the auth renewal, the worker and its events,
//!   playtime and the run log.
//! * [`install`] -- the plan and transfer of libraries, client jar, assets,
//!   natives, and pack archives.
//! * [`meta`] -- [`meta::PublisherMeta`], the metadata store a launch resolves
//!   through (loaders' own services, their installer jars, Prism's mirror).
//! * [`catalog`] -- Minecraft release and loader-build lists.
//! * [`java_runtime`] -- fetching the runtime a version names.
//! * [`loader_install`] -- Forge's and NeoForge's processors.
//! * [`wire`] -- the engine's cache, pool and download queue.
//! * [`accounts`] -- the account store the launch signs in from.
//! * [`model`] -- the override-gated settings view.
//! * [`instances`] and [`mods`] -- instance creation/discovery and the mod
//!   folder, which [`meta`] and [`instances`] own and whose tests build real
//!   instances.
//! * [`brand`] -- the product name and version the launch script and run log
//!   carry.
//!
//! Nothing in this crate depends on iced, wgpu or `windows-sys`. It is safe Rust
//! (`#![forbid(unsafe_code)]`), which is also why four things stayed behind.
//!
//! ## The seams with the shell
//!
//! * **Preferences.** `Prefs` is UI settings (it carries the colour theme and
//!   depends on the generated string table), so it stays in `sift-desktop`.
//!   [`launch::LaunchDefaults`] is plain data here; the shell's
//!   `launch::defaults_from_prefs` and `launch::java_prefs` read its fields.
//!   `LaunchDefaults::from_prefs` and `JavaPrefs::from_prefs` therefore no
//!   longer exist as methods -- this is a signature change, and the shell's
//!   tests for both moved with the conversion functions.
//! * **The window system.** A maximized instance is passed the primary monitor's
//!   work area, and that query is Win32. [`launch::prepare_launch`] and
//!   [`launch::run_launch_worker`] gained a `work_area: Option<WorkAreaProbe>`
//!   parameter -- a function pointer the shell supplies, called only when an
//!   instance actually asks to maximize. `sift-desktop`'s launch shim passes
//!   `native::primary_work_area`, so its three-argument `run_launch_worker` is
//!   unchanged for `shell.rs`; the egui shell passes its own monitor query or
//!   `None`. Nothing else about the launch changes.
//! * **Power management.** [`install::run`] and [`launch::run_launch_worker`] used
//!   to call `awake::Awake::begin()`, which is `SetThreadExecutionState` and
//!   therefore `unsafe` FFI. The guard now belongs to the caller, on the thread
//!   doing the work: `sift-desktop` asks at the two entry points it owns
//!   (`shell.rs`'s worker thread and `Store::repair_instance`), which is where
//!   the behaviour it shipped with lives. A shell that forgets it gets the old
//!   pre-guard behaviour (the machine may sleep through a long install), so the
//!   seam is documented in both places rather than being silent.
//! * **Branding.** [`brand`] carries only the product name and version; the
//!   artwork, window icon and About page stay in `sift-desktop/src/brand.rs`,
//!   which re-exports [`brand::APP_NAME`] and [`brand::version`].
//!
//! The launch worker still speaks [`launch::LaunchEvent`] down a
//! `futures::channel::mpsc::Sender`, which is a futures type rather than an iced
//! one: iced's subscription wraps it, and an egui shell can pump the same stream
//! with `try_recv` on a thread. Progress reaches the installer through
//! `&mut dyn FnMut(install::Progress)`, not through any UI type.
//!
//! ## Policy
//!
//! No `unwrap`/`expect` outside tests, no `unsafe`, no UI dependencies. The
//! tests that moved with the code are unchanged except where a test was *about* a
//! seam: the preference conversions' two tests stayed in `sift-desktop` with the
//! functions they test, the power guard's source check in `sift-desktop/src/awake.rs`
//! now points at the shell's call sites, and the launch tests pass one extra
//! `None` for the work-area probe. Their assertions are the ones that ran before.

// The policy below is "no `unwrap`/`expect` outside tests", so tests have to be
// exempt or the crate cannot compile under clippy at all. The `cfg_attr(test,
// allow)` line comes after the deny because the last applicable level wins --
// the order `sift-core` uses, and the only one that actually exempts tests.
#![deny(clippy::unwrap_used, clippy::expect_used)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]
#![forbid(unsafe_code)]

pub mod accounts;
pub mod brand;
pub mod catalog;
pub mod install;
pub mod instances;
pub mod java_runtime;
pub mod launch;
pub mod loader_install;
pub mod meta;
pub mod model;
pub mod mods;
pub mod wire;
