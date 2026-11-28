//! The entity/component store of record. Scene-level state lives in the root's `scene::Scene`,
//! which owns a `World`. Consumers go through the accessors, never the hecs storage. An owned
//! `Entity` is only the document (the saved shape): build, serialize or diff one anywhere.

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
pub mod changes;
pub mod core;
mod descendants;
#[cfg(test)]
mod descendants_tests;
mod despawn;
mod funnel;
mod handles;
pub mod journal;
#[cfg(test)]
mod journal_tests;
mod names;
#[cfg(test)]
mod names_tests;
mod placement;
#[cfg(test)]
mod placement_tests;
#[cfg(test)]
mod query_tests;
#[cfg(test)]
mod query_where_tests;
#[cfg(test)]
mod static_revision_tests;
#[doc(hidden)]
pub mod walks;
pub mod world;
mod world_space;

pub use access::{CompMut, CompRef};
pub use changes::{
    Activity, ChangeStat, Changes, Channel, Children, Cursor, Cut, Layer, StaticGeometry,
    StaticSet, WorldPose,
};
pub use descendants::{Descendants, HierarchyPath};
pub use journal::{Journaled, Recorded};
pub use world::World;
pub use world_space::{ColliderMut, TransformMut};

/// The id no entity has: ids start at 1 (`World::new`, `World::clear`). The script API reads a
/// `nil` id as it.
pub const NO_ENTITY: u32 = 0;
