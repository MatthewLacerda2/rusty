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
mod active_tests;
mod activity;
#[cfg(test)]
mod activity_tests;
#[cfg(test)]
mod ancestors_tests;
mod bundle;
pub mod core;
mod descendants;
#[cfg(test)]
mod descendants_tests;
mod despawn;
#[cfg(test)]
mod despawn_tests;
mod handles;
#[cfg(test)]
mod query_tests;
pub mod world;
mod world_space;

pub use access::{CompMut, CompRef};
pub use descendants::Descendants;
pub use world::World;
pub use world_space::{ColliderMut, TransformMut};

/// The id no entity has: the allocator hands out ids from 1 (`World::new`,
/// `World::clear`), so 0 names nothing. The API reads a `nil` id as it (#1383).
pub const NO_ENTITY: u32 = 0;
