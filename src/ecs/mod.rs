//! src/ecs/mod.rs — ECS facade
//!
//! Wraps `hecs::World` with the engine's stable-id + name lookup layer. The
//! `World` here is THE entity/component store of record. Scene-level state
//! (selection, skybox, ambient light) and the cross-world helpers live in
//! `crate::scene::Scene`, which owns a `World` — kept out of here so this layer
//! stays restricted to its allowed deps.
//!
//! Allowed deps: hecs, components.

pub mod world;

pub use world::World;
