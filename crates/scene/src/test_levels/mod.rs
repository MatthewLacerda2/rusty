//! crates/scene/src/test_levels/mod.rs — test hooks: plain primitive levels other
//! crates' tests and benches build (`docs/testing.md`, *Test content is primitive*).
//!
//! A level here bakes nothing and names no crate above the base, so any crate's
//! tests and benches can build one: the occlusion and lightmap bakes'
//! (`rusty-lighting`), the navmesh's and the AI's query bench (`rusty-ai`).

use glam::Vec3;

use crate::create::{create_entity, Primitive};
use crate::Scene;

#[cfg(feature = "dev")]
pub mod rooms;

/// A static box over `[min, max]`.
pub fn block(scene: &mut Scene, name: &str, min: Vec3, max: Vec3) -> u32 {
    let id = create_entity(scene, name, Some(Primitive::Box));
    let mut t = scene.world.transform_mut(id).expect("a new entity");
    (t.position, t.scale) = ((min + max) * 0.5, max - min);
    drop(t);
    scene.world.set_static(id, true);
    id
}
