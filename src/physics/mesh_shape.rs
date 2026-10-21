//! src/physics/mesh_shape.rs — a mesh collider's rapier shape (#77).
//!
//! The one shape [`super::build::build_shape`] builds from geometry rather than
//! dimensions, so the one that a mirrored world scale must reflect (#1124).

use glam::Vec3;
use rapier3d::prelude::*;

/// Build a trimesh or convex-hull collider from rest-pose mesh geometry (#77),
/// baking the signed world scale into each point. A mirror (odd number of negative
/// axes) would turn every triangle inside out, so it swaps two indices of each
/// (#1124). `None` if the geometry is too sparse to form a valid shape.
pub(super) fn build_mesh_shape(
    convex: bool,
    scale: Vec3,
    mesh: (&[[f32; 3]], &[u32]),
) -> Option<Collider> {
    let (positions, indices) = mesh;
    if positions.len() < 4 {
        return None;
    }
    let points: Vec<Vector> = positions
        .iter()
        .map(|p| Vector::new(p[0] * scale.x, p[1] * scale.y, p[2] * scale.z))
        .collect();

    if convex {
        // A convex hull is the cheaper, dynamic-body-friendly option.
        ColliderBuilder::convex_hull(&points).map(|b| b.build())
    } else {
        // An exact triangle mesh — static geometry only, but a tight fit.
        let mirrored = scale.x * scale.y * scale.z < 0.0;
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
            return None;
        }
        ColliderBuilder::trimesh(points, triangles)
            .ok()
            .map(|b| b.build())
    }
}
