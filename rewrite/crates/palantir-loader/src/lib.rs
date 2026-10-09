//! Installing game versions, and the loaders that run on them.
//!
//! A "loader install" is not a different kind of artifact: vanilla, Fabric,
//! Forge, NeoForge and Quilt all end as one *version document* on disk
//! (`versions/<id>/<id>.json`), the libraries and jars it names, and an
//! asset index -- exactly what `palantir-core` describes and `palantir-net`
//! can fetch. What differs is how the document is obtained:
//!
//! - **Vanilla**: the document *is* the metadata; sync it and it is done.
//! - **Fabric / Quilt**: their meta services publish a launcher profile
//!   that names `inheritsFrom`; merge it over the game's document with
//!   `Version::merged_with` and sync the result's needs.
//! - **Forge / NeoForge**: the vendor's installer jar carries the version
//!   document (and, on older generations, a processor pipeline that must
//!   run Java to produce the patched artifacts).
//!
//! The importers translate foreign *packagings* into the same shape:
//! Modrinth's `.mrpack`, Prism/MultiMC's `instance.cfg` +
//! `mmc-pack.json`, CurseForge's `manifest.json` (the export format the
//! CurseForge app, GDLauncher and ATLauncher all share), and the vanilla
//! `.minecraft` layout every other launcher -- official, TLauncher,
//! SKLauncher, Badlion, Legacy, Lunar, Feather -- keeps around its own
//! roots. All resolve to "a game version, a loader, a set of files" --
//! which is an install.
//!
//! What this crate does not do: anything windowed (that is `palantir-desktop`
//! in its own time), or own the formats (that is `palantir-core`) or the
//! transfer (that is `palantir-net`).
//!
//! The phase's done-when, when the pieces below land: each loader installs
//! into a temp data root and the result launches headless Java -- the
//! fetched runtime starts the game process from the built launch plan and
//! the test can watch it run.
//!
//! The map, in the order the slices are landing:
//!
//! 1. `install` -- vanilla: a version's needs into a data root (the
//!    `Syncer` composition) with its resolved document written out. **Landed.**
//! 2. `java` -- the Java runtime index and manifest: choosing the
//!    platform's runtime and placing its files. **Landed** (the download
//!    lands with `launch`).
//! 3. `profiles` -- Fabric/Quilt listings and launcher profiles,
//!    installed over the game document. **Landed.**
//! 4. `installer` -- the Forge/NeoForge install profile and its processor
//!    pipeline, planned with every token expanded. **Landed** (running
//!    the processors lands with `launch`).
//! 5. `import`, `mrpack`, `prism`, `curseforge`, `vanilla` -- the
//!    importers, one per packaging, all translating to `import`'s shape.
//!    **Landed.**
//! 6. `launch` -- runtime resolution, natives extraction, and the process.

pub mod curseforge;
pub mod error;
pub mod import;
pub mod install;
pub mod installer;
pub mod java;
pub mod mrpack;
pub mod prism;
pub mod profiles;
pub mod vanilla;
