//! src/physics/wire/ — a collider's shape as world-space line segments, for the
//! collider gizmo (#1882): Unity's green wireframe of the shape physics collides
//! with, not of its bounds.
//!
//! Primitives are drawn as Unity outlines them, from the same scaled sizes the
//! rapier shape is built from ([`ColliderShape::scaled`]): a box's twelve edges, a
//! sphere's three great circles, a cylinder's two rims and four sides, a capsule's
//! two rims, four sides and its caps' arcs. A mesh collider is built through the
//! very checks physics builds it with (`shape::build::build_shape_for`), so a mesh
//! physics refuses (a bad index, no area) draws nothing, as it collides with
//! nothing. A convex mesh draws its hull's edges (parry merges coplanar faces, so a
//! box hull shows twelve, not its diagonals); a triangle mesh draws each edge of
//! the built mesh once, by position, so a seam split for its normals draws once.
//! A CharacterController's capsule draws too, upright at its centre, as physics
//! places it. A concave mesh on a dynamic body is built as convex pieces (#1886),
//! and draws as those pieces.
//!
//! **Where the drawn and the collided shape differ.** The wire is the Collider as
//! it stands; in Play, rapier reads a primitive's shape only when the body is built
//! (entering Play, a hierarchy change: `docs/api/Physics.md`). So after
//! `Physics.SetColliderShape` the wire shows the new shape before physics collides
//! with it, until that body is built again.
//!
//! **Built once, placed every frame.** The shape, in its own space, is the costly
//! half (a hull, a level's trimesh); [`WireCache`] keeps it per entity and builds it
//! again only when the inputs physics builds from change. Placing it at the
//! entity's pose is the per-frame half. A shape is held to [`MAX_SEGMENTS`].

mod cache;
mod edges;

use glam::{Quat, Vec3};

use super::compound::body_owner;
use super::shape::build::{class_of, collider_inputs, BodyClass};
use super::shape::fit::build_shape_for;
use crate::components::transform::Pose;
use crate::components::ScaledShape;
use crate::scene::Scene;
pub use cache::WireCache;

/// The most segments one collider's wire holds: a level's collision mesh past it
/// draws its first ones.
pub const MAX_SEGMENTS: usize = 32_768;

/// One collider's wireframe in world space.
#[derive(Debug, Default)]
pub struct ColliderWire {
    /// World-space segments, each its two ends.
    pub segments: Vec<[Vec3; 2]>,
    /// The collider is a trigger (Unity draws it the same, paler here).
    pub trigger: bool,
}

/// One collider's wireframe in its own space, scaled, before its pose places it.
#[derive(Debug)]
pub struct ShapeWire {
    /// The segments, centred and unrotated.
    pub segments: Vec<[Vec3; 2]>,
    /// The collider is a trigger.
    pub trigger: bool,
    /// A CharacterController's capsule: upright, at `center` (#451).
    upright: bool,
    center: Vec3,
}

/// `id`'s active collider (or CharacterController) in its own space, or `None`
/// when it has neither, or a mesh physics builds no shape from (logged as physics
/// logs it).
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

/// Whether `id`'s collider joins a dynamic body: a concave mesh is then built as
/// convex pieces (#1886), so its wire is too.
pub fn on_dynamic_body(scene: &Scene, id: u32) -> bool {
    let owner = body_owner(scene, id);
    matches!(class_of(&scene.world, owner), BodyClass::Dynamic)
}

impl ShapeWire {
    /// Its segments at `id`'s world pose, as physics places the shape; `None` when
    /// the entity is gone.
    pub fn placed<'a>(
        &'a self,
        scene: &Scene,
        id: u32,
    ) -> Option<impl Iterator<Item = [Vec3; 2]> + 'a> {
        Some(self.placed_at(scene.world.world_pose(id)?))
    }

    /// Its segments at `pose`, its entity's world pose (or the pose a frame draws
    /// it at, blended between two ticks), as physics places the shape there.
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

/// `id`'s collider as world-space segments, built afresh (no cache): what
/// [`WireCache`] gives, placed.
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
