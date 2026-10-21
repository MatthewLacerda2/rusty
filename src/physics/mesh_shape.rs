//! src/physics/mesh_shape.rs — a mesh collider's rapier shape (#77).
//!
//! The one shape [`super::build::build_shape`] builds from geometry rather than
//! dimensions, so the one that a mirrored world scale must reflect (#1124).

use glam::Vec3;
use rapier3d::prelude::*;

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
            "mesh collider skipped: {} vertices, a {} needs {min_points}",
            positions.len(),
            if convex { "convex hull" } else { "trimesh" }
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
            log::warn!("mesh collider skipped: the points form no convex hull");
        }
        hull
    } else {
        // An exact triangle mesh — static geometry only, but a tight fit.
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
            log::warn!("mesh collider skipped: a trimesh needs at least one triangle");
            return None;
        }
        ColliderBuilder::trimesh(points, triangles)
            .map_err(|e| log::warn!("mesh collider skipped: {e}"))
            .ok()
            .map(|b| b.build())
    }
}
