//! Re-export of [`sift_game::wire`]: the one way a launch reaches the network.
//!
//! The cache, pool and queue moved with the engine so both shells transfer
//! through the same code. The engine's scripted test service stays in
//! `sift-game` (it is `#[cfg(test)]` there) and is deliberately not re-exported:
//! the tests that drive it moved with it.
pub use sift_game::wire::*;
