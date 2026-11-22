use glam::Vec3;
use rapier3d::prelude::*;

use super::build::try_build_body_shape;
use rusty_components::ColliderShape;

pub(crate) fn build_shape_for(
    id: u32,
    shape: &ColliderShape,
    scale: Vec3,
    mesh: Option<(&[[f32; 3]], &[u32])>,
    dynamic: bool,
) -> Option<Collider> {
    let mut collider = try_build_body_shape(shape, scale, mesh, dynamic)
        .map_err(|skip| log::warn!("[Physics] entity {id}: mesh collider skipped: {skip}"))
        .ok()?;
    collider.user_data = fit_mark(shape, dynamic);
    if collider.user_data == DECOMPOSED && collider.shape().as_trimesh().is_some() {
        log::warn!(
            "[Physics] entity {id}: concave mesh collider found no convex pieces; \
             kept as a triangle mesh, which has no volume on a dynamic body"
        );
    }
    Some(collider)
}

// The fit a collider was built for, stamped in its rapier `user_data`: `live`'s sync
// rebuilds the shape when the body's class in Play wants another. Zero: the same on
// any body.
pub(crate) const DECOMPOSED: u128 = 2;
pub(crate) const EXACT: u128 = 1;
pub(crate) const ROLLING: u128 = 3;

pub(crate) fn fit_mark(shape: &ColliderShape, dynamic: bool) -> u128 {
    match shape {
        ColliderShape::Mesh { convex: false, .. } if dynamic => DECOMPOSED,
        ColliderShape::Cylinder { .. } | ColliderShape::Capsule { .. } if dynamic => ROLLING,
        ColliderShape::Mesh { convex: false, .. }
        | ColliderShape::Cylinder { .. }
        | ColliderShape::Capsule { .. } => EXACT,
        _ => 0,
    }
}
