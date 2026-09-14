//! # palantir-loader
//!
//! Phase 3 modloader/modpack support for PalantirMC:
//! safe archive extraction, Modrinth/CurseForge import, JAR scanning and
//! component override patches.
//!
//! Phase-1 hooks it builds on: [`palantir_core::pack::PackProfile`] (component
//! install/customize), [`palantir_core::version::VersionFile`] (patch
//! serialization) and the `patches/<uid>.json` layout of
//! [`palantir_core::instance::Instance`].

#![deny(clippy::unwrap_used, clippy::expect_used)]
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used))]
#![warn(missing_docs)]

pub mod archive;
pub mod install;
pub mod jar;
pub mod modpack;

pub use archive::{
    extract_tar_gz, extract_tar_gz_bytes, extract_tar_gz_file, extract_zip, extract_zip_bytes,
    extract_zip_file, extract_zip_file_flat,
};
pub use install::write_patch;
pub use jar::{scan_jar, scan_jar_file, JarInfo};
pub use modpack::{
    detect_format, import_curseforge, import_mrpack, plan_pack, PackFile, PackFormat, PackPlan,
};
