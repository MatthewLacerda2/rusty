use glam::Vec3;
use rapier3d::prelude::{ConvexPolyhedron, Pose};

/// Rounding a face plane may leave, as a fraction of the sizes in play.
pub(crate) const SLACK: f32 = 1e-5;

fn planes(hull: &ConvexPolyhedron) -> impl Iterator<Item = (Vec3, Vec3)> + '_ {
    let corner = |first: u32| hull.points()[hull.vertices_adj_to_face()[first as usize] as usize];
    (hull.faces().iter()).map(move |f| (f.normal, corner(f.first_vertex_or_edge)))
}

/// Whether the hull encloses a volume: every face has a point behind it. parry
/// builds coplanar points (a flat mesh, a zero scale axis) as a double-sided fan,
/// whose two face planes hold every point of their plane, so its planes answer wrong.
pub(crate) fn has_volume(hull: &ConvexPolyhedron) -> bool {
    let size = hull.local_aabb().extents().max_element();
    let width = |(normal, on): (Vec3, Vec3)| {
        (hull.points().iter())
            .map(|&p| normal.dot(on - p))
            .fold(0.0, f32::max)
    };
    hull.faces().len() >= 4 && planes(hull).all(|plane| width(plane) > SLACK * size)
}

/// How far `point` lies past the farthest face plane, zero or less inside. Outside it
/// is only a lower bound: near an edge or corner the true distance is larger, unbounded.
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
