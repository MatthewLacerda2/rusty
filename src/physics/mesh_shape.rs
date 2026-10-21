//! src/physics/mesh_shape.rs — a mesh collider's rapier shape (#77).
//!
//! The one shape [`super::build::build_shape`] builds from geometry rather than
//! dimensions, so the one that a mirrored world scale must reflect (#1124).

use glam::Vec3;
use rapier3d::prelude::*;

use super::build::build_shape;
use crate::components::ColliderShape;

/// [`build_shape`] for collider entity `id`: a mesh collider that is skipped says
/// which entity lost it (#1141). The reason itself is logged by [`build_mesh_shape`].
pub(super) fn build_shape_for(
    id: u32,
    shape: &ColliderShape,
    scale: Vec3,
    mesh: Option<(&[[f32; 3]], &[u32])>,
) -> Option<Collider> {
    let built = build_shape(shape, scale, mesh);
    if built.is_none() && mesh.is_some() && matches!(shape, ColliderShape::Mesh { .. }) {
        log::warn!("[Physics] entity {id} has no collider: its mesh collider was skipped");
    }
    built
}

/// Build a trimesh or convex-hull collider from rest-pose mesh geometry (#77),
/// baking the signed world scale into each point. A mirror (odd number of negative
/// axes) would turn every triangle inside out, so it swaps two indices of each
/// (#1124). `None`, with a warning, if the geometry is too sparse to form a valid
/// shape: a hull needs 4 vertices, a trimesh one triangle (#1132).
pub(super) fn build_mesh_shape(
    convex: bool,
    scale: Vec3,
    mesh: (&[[f32; 3]], &[u32]),
) -> Option<Collider> {
    let (positions, indices) = mesh;
    let min_points = if convex { 4 } else { 3 };
    if positions.len() < min_points {
        log::warn!(
            "[Physics] mesh collider skipped: {} vertices, a {} needs {min_points}",
            positions.len(),
            if convex { "convex hull" } else { "trimesh" }
        );
        return None;
    }
    if let Some(bad) = indices.iter().find(|&&i| i as usize >= positions.len()) {
        log::warn!(
            "[Physics] mesh collider skipped: index {bad} is past the {} vertices",
            positions.len()
        );
        return None;
    }
    let points: Vec<Vector> = positions
        .iter()
        .map(|p| Vector::new(p[0] * scale.x, p[1] * scale.y, p[2] * scale.z))
        .collect();

    if convex {
        // A convex hull is the cheaper, dynamic-body-friendly option.
        let hull = ColliderBuilder::convex_hull(&points).map(|b| b.build());
        if hull.is_none() {
            log::warn!("[Physics] mesh collider skipped: the points form no convex hull");
        }
        hull
    } else {
        trimesh(points, indices, scale)
    }
}

/// An exact triangle mesh — static geometry only, but a tight fit. Mirrors the
/// winding for a reflecting `scale`; skips an empty or all-degenerate mesh.
fn trimesh(points: Vec<Vector>, indices: &[u32], scale: Vec3) -> Option<Collider> {
    // Counted, not multiplied: a product of tiny axes can underflow to -0.0.
    let mirrored = (scale.x < 0.0) ^ (scale.y < 0.0) ^ (scale.z < 0.0);
    let triangles: Vec<[u32; 3]> = indices
        .chunks_exact(3)
        .map(|c| {
            if mirrored {
                [c[0], c[2], c[1]]
            } else {
                [c[0], c[1], c[2]]
            }
        })
        .collect();
    if triangles.is_empty() {
        log::warn!("[Physics] mesh collider skipped: a trimesh needs at least one triangle");
        return None;
    }
    if triangles.iter().all(|t| is_degenerate(&points, t)) {
        log::warn!("[Physics] mesh collider skipped: every triangle has zero area");
        return None;
    }
    ColliderBuilder::trimesh(points, triangles)
        .map_err(|e| log::warn!("[Physics] mesh collider skipped: {e}"))
        .ok()
        .map(|b| b.build())
}

/// A triangle whose area is nil next to the mesh's size: its edge cross product is
/// below `1e-6` of the squared extent (#1141).
fn is_degenerate(points: &[Vector], t: &[u32; 3]) -> bool {
    let p = |i: u32| points[i as usize];
    let (a, b, c) = (p(t[0]), p(t[1]), p(t[2]));
    let extent = points.iter().fold(0.0_f32, |m, v| {
        m.max(v.x.abs()).max(v.y.abs()).max(v.z.abs())
    });
    (b - a).cross(c - a).length() <= 1e-6 * extent * extent
}
