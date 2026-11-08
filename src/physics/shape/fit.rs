//! src/physics/shape/fit.rs — a collider entity's shape as its body needs it.
//!
//! A concave mesh collider is the one shape that depends on its body: the exact
//! trimesh on a static or kinematic body, convex pieces on a dynamic one (#1886).
//! This builds a collider for an entity, logs why one was not built or not split,
//! and stamps the fit it was built for, which the live sync reads when a body
//! changes type in Play.

use glam::Vec3;
use rapier3d::prelude::*;

use super::build::try_build_body_shape;
use crate::components::ColliderShape;

/// [`try_build_body_shape`] for collider entity `id` on a body that is `dynamic` or
/// not: a skipped mesh collider is logged once, naming the entity and the reason
/// (#1141), and so is a concave one on a dynamic body that V-HACD could not split,
/// kept as a trimesh (#1886). The collider's [`fit_mark`] records the fit asked for.
pub(in crate::physics) fn build_shape_for(
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

/// A concave mesh collider built for a dynamic body (decomposed), in its rapier
/// collider's `user_data`: the fit `live::sync_mesh_fits` compares with the body's
/// class, to rebuild the shape when a body changes type in Play.
pub(in crate::physics) const DECOMPOSED: u128 = 2;
/// A concave mesh collider built for a static or kinematic body (exact).
pub(in crate::physics) const EXACT: u128 = 1;

/// The `user_data` [`build_shape_for`] stamps: [`DECOMPOSED`], [`EXACT`], or zero
/// for every collider whose shape is the same on any body.
pub(in crate::physics) fn fit_mark(shape: &ColliderShape, dynamic: bool) -> u128 {
    match shape {
        ColliderShape::Mesh { convex: false, .. } if dynamic => DECOMPOSED,
        ColliderShape::Mesh { convex: false, .. } => EXACT,
        _ => 0,
    }
}
