//! **Where the drawn and the collided shape differ.** The wire is the Collider as
//! it stands; in Play, rapier reads a primitive's shape only when the body is built
//! (entering Play, a hierarchy change: `docs/api/Physics.md`). So after
//! `Physics.SetColliderShape` the wire shows the new shape before physics collides
//! with it, until that body is built again.

mod cache;
mod edges;

use glam::{Quat, Vec3};

use super::compound::body_owner;
use super::shape::build::{class_of, collider_inputs, BodyClass};
use super::shape::fit::build_shape_for;
pub use cache::WireCache;
use rusty_components::transform::Pose;
use rusty_components::ScaledShape;
use rusty_scene::Scene;

/// The most segments one collider's wire holds: a level's collision mesh past it draws its first.
pub const MAX_SEGMENTS: usize = 32_768;

#[derive(Debug, Default)]
pub struct ColliderWire {
    pub segments: Vec<[Vec3; 2]>,
    pub trigger: bool,
}

#[derive(Debug)]
pub struct ShapeWire {
    pub segments: Vec<[Vec3; 2]>,
    pub trigger: bool,
    upright: bool,
    center: Vec3,
}

pub fn shape_wire(scene: &Scene, id: u32) -> Option<ShapeWire> {
    let inp = collider_inputs(&scene.world, id)?;
    let scale = scene.world.world_pose(id)?.scale;
    let dynamic = on_dynamic_body(scene, id);
    let mut segments = match inp.shape.scaled(scale) {
        ScaledShape::Mesh { .. } => {
            let mesh = inp.mesh_geom.as_ref();
            let mesh = mesh.map(|(p, i)| (p.as_slice(), i.as_slice()));
            edges::built(&build_shape_for(id, &inp.shape, scale, mesh, dynamic)?)?
        }
        primitive => edges::primitive(primitive),
    };
    segments.truncate(MAX_SEGMENTS);
    Some(ShapeWire {
        segments,
        trigger: inp.is_trigger,
        upright: inp.upright,
        center: inp.center,
    })
}

pub fn on_dynamic_body(scene: &Scene, id: u32) -> bool {
    let owner = body_owner(scene, id);
    matches!(class_of(&scene.world, owner), BodyClass::Dynamic)
}

impl ShapeWire {
    pub fn placed<'a>(
        &'a self,
        scene: &Scene,
        id: u32,
    ) -> Option<impl Iterator<Item = [Vec3; 2]> + 'a> {
        Some(self.placed_at(scene.world.world_pose(id)?))
    }

    pub fn placed_at(&self, pose: Pose) -> impl Iterator<Item = [Vec3; 2]> + '_ {
        let (pos, rot) = match self.upright {
            true => (
                pose.pos + pose.rot * (self.center * pose.scale),
                Quat::IDENTITY,
            ),
            false => (pose.pos, pose.rot),
        };
        self.segments.iter().map(move |s| s.map(|p| pos + rot * p))
    }
}

/// Test hook for this crate's wire tests: `id`'s wire built afresh, no cache, then placed.
pub fn collider_wire(scene: &Scene, id: u32) -> Option<ColliderWire> {
    let shape = shape_wire(scene, id)?;
    let segments = shape.placed(scene, id)?.collect();
    Some(ColliderWire {
        segments,
        trigger: shape.trigger,
    })
}

#[cfg(test)]
mod cache_tests;
#[cfg(test)]
mod fit_tests;
#[cfg(test)]
mod tests;
