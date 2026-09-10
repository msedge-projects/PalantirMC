//! # prism-net (phase 2 stub)
//!
//! Will provide: Microsoft OAuth + Yggdrasil auth flows, the online
//! [`prism_core::resolve::MetaStore`] implementation over
//! `meta.prismlauncher.org`, library/asset download pipeline (tokio +
//! reqwest), and Modrinth/CurseForge API clients.
//!
//! The seam is already fixed by phase 1: implement
//! [`prism_core::resolve::MetaStore`] and plug it into
//! [`prism_core::resolve::resolve`].
