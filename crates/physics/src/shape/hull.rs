use glam::Vec3;
use rapier3d::prelude::{ConvexPolyhedron, Pose};

pub(crate) const SLACK: f32 = 1e-5;

fn planes(hull: &ConvexPolyhedron) -> impl Iterator<Item = (Vec3, Vec3)> + '_ {
    let corner = |first: u32| hull.points()[hull.vertices_adj_to_face()[first as usize] as usize];
    (hull.faces().iter()).map(move |f| (f.normal, corner(f.first_vertex_or_edge)))
}

/// Whether the hull encloses a volume: every face has a point behind it. parry
/// builds coplanar points (a flat mesh, or one at a zero scale axis) as a
/// double-sided fan, whose two face planes hold every point of their plane, so a
/// flat hull must not be answered by its planes. It costs one pass over the
pub(crate) fn has_volume(hull: &ConvexPolyhedron) -> bool {
    let size = hull.local_aabb().extents().max_element();
    let width = |(normal, on): (Vec3, Vec3)| {
        (hull.points().iter())
            .map(|&p| normal.dot(on - p))
            .fold(0.0, f32::max)
    };
    hull.faces().len() >= 4 && planes(hull).all(|plane| width(plane) > SLACK * size)
}

/// For a point outside, the result is only a lower bound on the distance to the surface: near
/// an edge or a corner the true distance is up to √3 times larger.
pub(crate) fn beyond_faces(hull: &ConvexPolyhedron, point: Vec3) -> f32 {
    planes(hull)
        .map(|(normal, on)| normal.dot(point - on))
        .fold(f32::MIN, f32::max)
}

pub(crate) fn holds(hull: &ConvexPolyhedron, pose: &Pose, point: Vec3) -> bool {
    let size = hull.local_aabb().extents().max_element();
    let local = pose.inverse_transform_point(point);
    beyond_faces(hull, local) <= SLACK * (size + point.length())
}
