//! # prism-loader
//!
//! Phase 3 modloader/modpack support for the Prism Launcher rewrite:
//! safe archive extraction, Modrinth/CurseForge import, JAR scanning and
//! loader patch planning.
//!
//! Phase-1 hooks it builds on: [`prism_core::pack::PackProfile`] (component
//! install/customize), [`prism_core::version::VersionFile`] (patch
//! serialization) and the `patches/<uid>.json` layout of
//! [`prism_core::instance::Instance`].

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
pub use install::{plan_loader_install, write_patch};
pub use jar::{scan_jar, scan_jar_file, JarInfo};
pub use modpack::{detect_format, import_curseforge, import_mrpack, PackFormat};
