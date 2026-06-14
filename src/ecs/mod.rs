//! src/ecs/mod.rs — ECS facade
//!
//! Wraps `hecs::World` with the engine's stable-id + name lookup layer. The
//! `World` here is the canonical entity store; the legacy `core::scene::Scene`
//! is now a thin façade over it during the migration.
//!
//! Allowed deps: hecs, components.

pub mod world;

pub use world::World;
