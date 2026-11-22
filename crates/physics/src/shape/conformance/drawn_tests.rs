use glam::Vec3;
use rapier3d::prelude::{Pose as RapierPose, Shape};

use super::bounds_tests::tolerance;
use super::{samples, turns, world_poses, KINDS, SCALES};
use crate::shape::build::build_shape;
use crate::shape::hull::{beyond_faces, has_volume};
use rusty_components::{CapsuleAxis, ColliderShape, Pose};
use rusty_scene::create::{primitive_geometry, Primitive};

type Drawn = (ColliderShape, Vec<[f32; 3]>, Option<Vec<u32>>);

fn drawn(kind: &ColliderShape) -> Vec<Drawn> {
    let primitive = |p, shape| {
        let (vertices, _) = primitive_geometry(p).expect("a mesh primitive");
        vec![(shape, vertices.iter().map(|v| v.position).collect(), None)]
    };
    match kind {
        ColliderShape::Box { .. } => {
            primitive(Primitive::Box, ColliderShape::Box { size: Vec3::ONE })
        }
        ColliderShape::Sphere { .. } => {
            primitive(Primitive::Sphere, ColliderShape::Sphere { radius: 0.5 })
        }
        ColliderShape::Cylinder { .. } => primitive(
            Primitive::Cylinder,
            ColliderShape::Cylinder {
                radius: 0.5,
                height: 2.0,
            },
        ),
        ColliderShape::Capsule { .. } => primitive(
            Primitive::Capsule,
            ColliderShape::Capsule {
                radius: 0.5,
                height: 2.0,
                axis: CapsuleAxis::Y,
            },
        ),
        ColliderShape::Mesh { .. } => samples(kind)
            .into_iter()
            .map(|s| {
                let (points, indices) = s.mesh.expect("a mesh sample carries geometry");
                (s.shape, points, Some(indices))
            })
            .collect(),
    }
}

/// How far `point` lies outside `shape` and from its surface. A hull's is a lower bound
/// ([`beyond_faces`]), so a point past the tolerance still fails.
fn outside_and_off(shape: &dyn Shape, pose: &RapierPose, point: Vec3) -> (f32, f32) {
    if let Some(hull) = shape.as_convex_polyhedron().filter(|h| has_volume(h)) {
        let beyond = beyond_faces(hull, pose.inverse_transform_point(point));
        return (beyond.max(0.0), beyond.abs());
    }
    let solid = shape.project_point(pose, point, true);
    let outside = if solid.is_inside {
        0.0
    } else {
        (solid.point - point).length()
    };
    let surface = shape.project_point(pose, point, false);
    (outside, (surface.point - point).length())
}

fn check(drawn: &Drawn, pose: Pose) {
    let (shape, points, indices) = drawn;
    let mesh = indices.as_ref().map(|i| (points.as_slice(), i.as_slice()));
    let collider = build_shape(shape, pose.scale, mesh).expect("builds");
    let at = RapierPose::from_parts(pose.pos, pose.rot);
    let aabb = collider.shape().compute_aabb(&at);
    let eps = tolerance(aabb.maxs - aabb.mins);
    let world = |p: [f32; 3]| pose.pos + pose.rot * (Vec3::from(p) * pose.scale);
    let (mut worst_out, mut nearest) = (0.0f32, f32::MAX);
    for &p in points {
        let (out, off) = outside_and_off(collider.shape(), &at, world(p));
        (worst_out, nearest) = (worst_out.max(out), nearest.min(off));
    }
    assert!(
        worst_out <= eps,
        "{shape:?} at {pose:?}: drawn {worst_out} outside what collides"
    );
    assert!(
        nearest <= eps,
        "{shape:?} at {pose:?}: drawn stops {nearest} short of what collides"
    );
}

/// Whether `shape` can hold its drawn mesh at `scale`: a capsule scaled wider than long
/// cannot (`docs/api/Physics.md`'s scale rule), so those scales are left out.
fn holds_at(shape: &ColliderShape, scale: Vec3) -> bool {
    let ColliderShape::Capsule { axis, .. } = shape else {
        return true;
    };
    let along = scale.abs().dot(axis.unit());
    (scale.abs() - along * axis.unit()).max_element() <= along
}

fn check_everywhere(drawn: &Drawn) {
    for scale in SCALES {
        for (turn, _) in turns() {
            for pose in world_poses(scale, turn) {
                if holds_at(&drawn.0, pose.scale) {
                    check(drawn, pose);
                }
            }
        }
    }
}

#[test]
fn what_is_drawn_is_what_collides() {
    KINDS
        .iter()
        .flat_map(drawn)
        .for_each(|d| check_everywhere(&d));
}
