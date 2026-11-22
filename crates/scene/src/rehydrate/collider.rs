use glam::Vec3;

use super::{bounds_of, hold_fields, install, rehydrate_one, Imports};
use rusty_components::{ColliderComponent, ColliderShape, SharedMesh};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Which {
    Kept,
    Collision,
}

impl Which {
    pub fn get(self, collider: &ColliderComponent) -> &Option<SharedMesh> {
        match self {
            Which::Kept => &collider.shared_mesh,
            Which::Collision => &collider.collision_mesh,
        }
    }

    pub fn of(self, collider: &mut ColliderComponent) -> &mut Option<SharedMesh> {
        match self {
            Which::Kept => &mut collider.shared_mesh,
            Which::Collision => &mut collider.collision_mesh,
        }
    }
}

pub(crate) fn rebuild(
    id: u32,
    collider: &mut ColliderComponent,
    which: Which,
    imports: &mut Imports,
) {
    if !matches!(collider.shape, ColliderShape::Mesh { .. }) {
        return;
    }
    let Some(shared) = which.of(collider) else {
        return;
    };
    let mut mesh = shared.reference();
    hold_fields(id, &mut mesh);
    shared.subdivisions = mesh.subdivisions;
    let rebuilt = rehydrate_one(&mesh, imports);
    install(&mut mesh, rebuilt);
    shared.take_geometry(&mesh);
}

/// The box follows physics' order (`physics::shape::build::collider_inputs`): the collision
/// mesh while it has triangles, else `own` (the entity's Mesh), else the kept copy.
pub fn refit_collider(collider: &mut ColliderComponent, own: Option<(Vec3, Vec3)>) {
    let points = |m: &SharedMesh| bounds_of(m.positions.iter().copied());
    let collision = collider.built_collision_mesh().and_then(points);
    let kept = || collider.shared_mesh.as_ref().and_then(points);
    if let Some(bounds) = collision.or(own).or_else(kept) {
        refit(collider, bounds);
    }
}

fn refit(collider: &mut ColliderComponent, (min, max): (Vec3, Vec3)) {
    if let ColliderShape::Mesh {
        local_min,
        local_max,
        ..
    } = &mut collider.shape
    {
        (*local_min, *local_max) = (min, max);
    }
}
