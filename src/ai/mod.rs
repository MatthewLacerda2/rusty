//! src/ai/ — how agents decide things about the world around them (#1892).
//!
//! Not navigation: navigation moves an agent to a point; this module chooses the
//! point. Today it holds the environment query ([`query`], Unreal's EQS), which
//! scores candidate spots by distance, facing, line of sight, the camera's view,
//! reachability and how far they sit from other agents. Perception or behaviour
//! trees would live here too if they come, as they sit in Unreal's `AIModule`.

// Panic-free sim (#195): bare `.unwrap()` is denied here (use `?` or a documented
// `.expect(...)`); test code is exempt via clippy.toml. See docs/linting.md.
#![deny(clippy::unwrap_used)]

pub mod query;
