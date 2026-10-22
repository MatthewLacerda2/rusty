//! src/physics/shape/hull.rs — a convex mesh collider measured by its face planes
//! (#1211).
//!
//! parry's point projection on a `ConvexPolyhedron` can call a point that sits
//! exactly on a hull vertex outside and answer a point far from it (#1195 took
//! that for a hull missing a cap). A face-plane test has no such breakdown, so the
//! point queries and the tests that measure a hull use this instead.

use glam::Vec3;
use rapier3d::prelude::{ConvexPolyhedron, Pose};

/// The rounding a hull's planes may leave, relative to the size of the numbers in
/// play: the hull's own and the query point's distance from the origin.
const SLACK: f32 = 1e-5;

/// How far `point`, in the hull's own space, lies beyond the hull's farthest face
/// plane: zero or less inside. For a point inside, minus its distance to the
/// surface (the nearest face plane of a convex shape is its nearest face).
pub(in crate::physics) fn beyond_faces(hull: &ConvexPolyhedron, point: Vec3) -> f32 {
    let corner = |first: u32| hull.points()[hull.vertices_adj_to_face()[first as usize] as usize];
    (hull.faces().iter())
        .map(|f| f.normal.dot(point - corner(f.first_vertex_or_edge)))
        .fold(f32::MIN, f32::max)
}

/// Whether the world-space `point` lies inside `hull` placed at `pose`, within
/// rounding: a point on a face, edge or corner is inside.
pub(in crate::physics) fn holds(hull: &ConvexPolyhedron, pose: &Pose, point: Vec3) -> bool {
    let size = hull.local_aabb().extents().max_element();
    let local = pose.inverse_transform_point(point);
    beyond_faces(hull, local) <= SLACK * (size + point.length())
}
