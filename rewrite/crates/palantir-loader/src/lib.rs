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
//! `.mrpack` (Modrinth's documented modpack format) and Prism/MultiMC's
//! `instance.cfg` + `mmc-pack.json` both resolve to "a game version, a
//! loader, a set of files" -- which is an install.
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
//!    `Syncer` composition), plus the Java runtime manifest and its
//!    download.
//! 2. `fabric`, `quilt` -- profile fetch + merge over the game document.
//! 3. `forge`, `neoforge` -- installer jar documents and the processor
//!    pipeline, run as headless Java.
//! 4. `mrpack`, `prism` -- the importers.
//! 5. `launch` -- runtime resolution, natives extraction, and the process.
