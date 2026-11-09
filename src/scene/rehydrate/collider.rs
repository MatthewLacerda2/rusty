//! src/scene/rehydrate/collider.rs — rebuild a Mesh-shaped collider's own meshes
//! from their references, and size its box to what it builds from.
//!
//! A collider carries two meshes of its own, saved as references only: the copy kept
//! when its entity's Mesh was removed (`shared_mesh`, #1657) and an authored
//! collision mesh (`collision_mesh`, #1960). Physics reads them in one order
//! (`physics::shape::build::collider_inputs`): the collision mesh while it has
//! triangles, else the entity's Mesh, else the kept copy. [`refit_collider`] sizes
//! the box by that same order.

use glam::Vec3;

use super::{bounds_of, hold_fields, install, rehydrate_one, reimport};
use crate::components::{ColliderComponent, ColliderShape, SharedMesh};

/// Which of a collider's own meshes to rebuild.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Which {
    /// `shared_mesh`: the copy kept of a removed Mesh.
    Kept,
    /// `collision_mesh`: the authored collision mesh.
    Collision,
}

impl Which {
    /// The field this names on `collider`.
    pub(crate) fn of(self, collider: &mut ColliderComponent) -> &mut Option<SharedMesh> {
        match self {
            Which::Kept => &mut collider.shared_mesh,
            Which::Collision => &mut collider.collision_mesh,
        }
    }
}

/// Rebuild a Mesh-shaped `collider`'s `which` mesh from its reference; another
/// shape's, or a collider without one, is left as it is. A reference that no longer
/// loads comes back empty, warned once (`rehydrate::warn_once_of`): an empty
/// collision mesh is not built from, so the collider falls back to its entity's Mesh.
pub(super) fn rebuild(id: u32, collider: &mut ColliderComponent, which: Which) {
    if !matches!(collider.shape, ColliderShape::Mesh { .. }) {
        return;
    }
    let Some(shared) = which.of(collider) else {
        return;
    };
    let mut mesh = shared.reference();
    hold_fields(id, &mut mesh);
    shared.subdivisions = mesh.subdivisions;
    let rebuilt = rehydrate_one(&mesh);
    install(&mut mesh, rebuilt);
    shared.take_geometry(&mesh);
}

/// [`rebuild`] `which`, then [`refit_collider`] with `own`, the entity's Mesh
/// bounds: what an undo does for a collider whose recorded mesh is not the live one.
pub(crate) fn rebuild_collider_mesh(
    id: u32,
    collider: &mut ColliderComponent,
    which: Which,
    own: Option<(Vec3, Vec3)>,
) {
    rebuild(id, collider, which);
    refit_collider(collider, own);
}

/// Size a Mesh-shaped collider's box to the geometry it builds from, as Unity's
/// `MeshCollider` follows its mesh: its collision mesh when that has triangles, else
/// `own` (its entity's Mesh bounds), else its kept shared mesh. With none of them,
/// or for any other shape, the box is left as it is.
pub(crate) fn refit_collider(collider: &mut ColliderComponent, own: Option<(Vec3, Vec3)>) {
    let points = |m: &SharedMesh| bounds_of(m.positions.iter().copied());
    let collision = collider.built_collision_mesh().and_then(points);
    let kept = || collider.shared_mesh.as_ref().and_then(points);
    if let Some(bounds) = collision.or(own).or_else(kept) {
        reimport::refit(collider, bounds);
    }
}
