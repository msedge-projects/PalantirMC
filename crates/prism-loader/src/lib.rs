//! # prism-loader (phase 3 stub)
//!
//! Will provide Forge/Fabric/NeoForge/Quilt detection, installer download
//! and patch generation, modpack import (zip/tar extraction with mmap for
//! large archives) and JAR scanning.
//!
//! Phase-1 hooks it builds on: [`prism_core::pack::PackProfile`] (component
//! install/customize), [`prism_core::version::VersionFile`] (patch
//! serialization) and the `patches/<uid>.json` layout of
//! [`prism_core::instance::Instance`].
