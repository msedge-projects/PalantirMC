//! # sift-game
//!
//! The launcher's game engine: the half that resolves a version, installs what
//! is missing, picks a Java and starts Minecraft. It exists so that the engine is
//! not owned by one UI: `crates/sift-ui` (egui, its own workspace) is the shell
//! now, and this crate is what lets it install and run a game. The iced shell it
//! was extracted from (`crates/sift-desktop`) has since been deleted.
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
//!   depends on the generated string table), so it belongs to the shell.
//!   [`launch::LaunchDefaults`] is plain data here; the shell's
//!   `defaults_from_prefs` and `java_prefs` read its fields.
//!   `LaunchDefaults::from_prefs` and `JavaPrefs::from_prefs` therefore no
//!   longer exist as methods -- this is a signature change, and a new shell needs
//!   its own conversion (the iced shell's lived in its launch shim and was deleted
//!   with it).
//! * **The window system.** A maximized instance is passed the primary monitor's
//!   work area, and that query is Win32. [`launch::prepare_launch`] and
//!   [`launch::run_launch_worker`] gained a `work_area: Option<WorkAreaProbe>`
//!   parameter -- a function pointer the shell supplies, called only when an
//!   instance actually asks to maximize. The iced shell's three-argument
//!   `run_launch_worker` shim passed `native::primary_work_area`; the egui shell
//!   passes its own monitor query or `None`. Nothing else about the launch
//!   changes.
//! * **Power management.** [`install::run`] and [`launch::run_launch_worker`] used
//!   to call `awake::Awake::begin()`, which is `SetThreadExecutionState` and
//!   therefore `unsafe` FFI. The guard now belongs to the caller, on the thread
//!   doing the work. The iced shell asked at the two entry points it owned
//!   (`shell.rs`'s worker thread and `Store::repair_instance`); `sift-ui` must do
//!   the same at its launch and repair entry points. A shell that forgets it gets
//!   the old pre-guard behaviour (the machine may sleep through a long install),
//!   so the seam is documented rather than silent.
//! * **Branding.** [`brand`] carries only the product name and version; the
//!   artwork, window icon and About page belong to the shell. The iced shell's
//!   `src/brand.rs` re-exported [`brand::APP_NAME`] and [`brand::version`]; the
//!   egui shell can do the same.
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
//! seam: the launch tests pass one extra `None` for the work-area probe, and the
//! two preference-conversion tests and the power guard's source check stayed with
//! the iced shell and were deleted with it. Their assertions are otherwise the
//! ones that ran before.

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
