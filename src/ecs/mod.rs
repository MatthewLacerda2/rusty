//! src/ecs/mod.rs — ECS facade
//!
//! Wraps `hecs::World` with the engine's stable-id + name lookup layer. The
//! `World` here is THE entity/component store of record. Scene-level state
//! (selection, skybox, ambient light) and the cross-world helpers live in
//! `crate::scene::Scene`, which owns a `World` — kept out of here so this layer
//! stays restricted to its allowed deps.
//!
//! This module is also the component-accessor facade (#344): `access.rs` holds
//! the per-component accessors, `core.rs` the identity-core + document verbs.
//! Consumers never touch storage directly — they go through these accessors.
//! Storage itself is real per-component hecs columns (#345); `Entity`
//! (`components::entity`) survives only as the on-disk document shape that
//! `write_components`/`entity_document` decompose/recompose against.

// Panic-free sim (#195, #962): bare `.unwrap()` is denied in every sim module (use `?`
// or a documented `.expect(...)`); test code is exempt via clippy.toml. See docs/linting.md.
#![deny(clippy::unwrap_used)]

pub mod access;
#[cfg(test)]
mod ancestors_tests;
mod bundle;
pub mod core;
mod descendants;
#[cfg(test)]
mod descendants_tests;
mod epoch;
mod handles;
#[cfg(test)]
mod query_tests;
pub mod world;

pub use access::{CompMut, CompRef};
pub use descendants::Descendants;
pub use epoch::Epoch;
pub use world::World;
