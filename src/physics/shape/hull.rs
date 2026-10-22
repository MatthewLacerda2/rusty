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

/// The face planes of `hull`, each as its normal and a point it passes through.
fn planes(hull: &ConvexPolyhedron) -> impl Iterator<Item = (Vec3, Vec3)> + '_ {
    let corner = |first: u32| hull.points()[hull.vertices_adj_to_face()[first as usize] as usize];
    (hull.faces().iter()).map(move |f| (f.normal, corner(f.first_vertex_or_edge)))
}

/// Whether the hull encloses a volume: every face has a point behind it. parry
/// builds coplanar points (a flat mesh, or one at a zero scale axis) as a
/// double-sided fan, whose two face planes hold every point of their plane, so a
/// flat hull must not be answered by its planes. It costs one pass over the
/// points per face.
pub(in crate::physics) fn has_volume(hull: &ConvexPolyhedron) -> bool {
    let size = hull.local_aabb().extents().max_element();
    let width = |(normal, on): (Vec3, Vec3)| {
        (hull.points().iter())
            .map(|&p| normal.dot(on - p))
            .fold(0.0, f32::max)
    };
    hull.faces().len() >= 4 && planes(hull).all(|plane| width(plane) > SLACK * size)
}

/// How far `point`, in the hull's own space, lies beyond the hull's farthest face
/// plane: zero or less inside. For a point inside, minus its distance to the
/// surface (the nearest face plane of a convex shape is its nearest face). For a
/// point outside it is only a lower bound on the distance to the surface: near
/// an edge or a corner the true distance is up to √3 times larger.
pub(in crate::physics) fn beyond_faces(hull: &ConvexPolyhedron, point: Vec3) -> f32 {
    planes(hull)
        .map(|(normal, on)| normal.dot(point - on))
        .fold(f32::MIN, f32::max)
}

/// Whether the world-space `point` lies inside `hull` placed at `pose`, within
/// rounding: a point on a face, edge or corner is inside. Only for a hull that
/// [`has_volume`].
pub(in crate::physics) fn holds(hull: &ConvexPolyhedron, pose: &Pose, point: Vec3) -> bool {
    let size = hull.local_aabb().extents().max_element();
    let local = pose.inverse_transform_point(point);
    beyond_faces(hull, local) <= SLACK * (size + point.length())
}
