//! The launcher's network layer: getting bytes and metadata, reliably.
//!
//! Five jobs, one crate:
//!
//! - [`client`] -- one pooled HTTP client (the spec names a blocking one);
//! - [`auth`] -- Microsoft account, Xbox Live and Minecraft sign-in, written
//!   from the protocols and taking its client id as a parameter, so a tree
//!   with no registered client id reports the feature unavailable instead of
//!   showing a code it invented;
//! - [`cache`] -- a TTL'd cache for the small metadata documents that move
//!   rarely, so opening the launcher twice does not ask the network twice;
//! - [`scheduler`] -- a global concurrency limit, because an install is
//!   thousands of files and services throttle the greedy;
//! - [`download`] -- transfers that survive interruption: resumable by
//!   HTTP range, retried with backoff on transient failures, and verified
//!   against the hash the metadata carries before anything is called done.
//!
//! On top, [`store`] keeps one verified copy per content hash and
//! [`sync`] drives all of it for a whole version. Everything here is
//! written from the protocols' own specifications; nothing is derived from
//! another launcher's implementation.

// A launcher that panics is a crash with no message. Lint attributes apply
// in sequence and the last one wins, so the test allowance must come AFTER
// the deny: before it, the deny cancels the allowance.
#![deny(clippy::expect_used, clippy::unwrap_used)]
#![cfg_attr(test, allow(clippy::expect_used, clippy::unwrap_used))]

pub mod auth;
pub mod cache;
pub mod client;
pub mod download;
pub mod error;
pub mod scheduler;
pub mod session;
pub mod store;
pub mod sync;

pub use error::{Error, Result};
