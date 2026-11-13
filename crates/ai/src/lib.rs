//! crates/ai/src/lib.rs — how agents decide things about the world around them (#1892).
//!
//! Not navigation: navigation moves an agent to a point; this module chooses the
//! point and senses what is around it, as Unreal's `AIModule` does: the environment
//! query ([`query`], Unreal's EQS) scores candidate spots by distance, facing, line
//! of sight, the camera's view, reachability and how far they sit from other
//! agents; [`perception`] (Unreal's AI Perception, #1900) steps every
//! `AIPerception`'s sight, hearing and damage senses, so an agent acts on what it
//! knows. Both trace sight through [`ray`] and read where an entity stands
//! from [`stand`].

// Panic-free sim (#195): bare `.unwrap()` is denied here (use `?` or a documented
// `.expect(...)`); test code is exempt via clippy.toml. See docs/linting.md.
#![deny(clippy::unwrap_used)]

pub mod perception;
pub mod query;
pub mod ray;
pub mod stand;
