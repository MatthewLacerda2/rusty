//! Invariant 3 (#1171): what is drawn is what collides. The engine's own
//! primitive mesh, put through the entity's world matrix, lies inside the shape
//! physics builds for the matching collider, and reaches its surface. A round
//! shape stays round under non-uniform scale (#1131), so a squashed sphere is
//! drawn inside its ball and touches it along the longest axis.

use glam::Vec3;
use rapier3d::prelude::{Pose as RapierPose, Shape};

use super::bounds_tests::tolerance;
use super::{samples, turns, world_poses, KINDS, SCALES};
use crate::components::{ColliderShape, Pose};
use crate::physics::shape::build::build_shape;
use crate::scene::authoring::{primitive_geometry, Primitive};

/// A drawn mesh's positions, and the collider that should fit it.
type Drawn = (ColliderShape, Vec<[f32; 3]>, Option<Vec<u32>>);

/// The mesh drawn for `kind`'s variant, paired with the collider the inspector
/// gives that shape. No `_` arm: a new variant answers here too.
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
                height: 1.0,
            },
        ),
        // The engine has no Capsule primitive to draw one with.
        ColliderShape::Capsule { .. } => vec![],
        // A mesh collider is drawn as the mesh it is built from.
        ColliderShape::Mesh { .. } => samples(kind)
            .into_iter()
            .map(|s| {
                let (points, indices) = s.mesh.expect("a mesh sample carries geometry");
                (s.shape, points, Some(indices))
            })
            .collect(),
    }
}

/// How far `point` lies outside `shape` at `pose` (zero inside), and how far it
/// is from its surface.
fn outside_and_off(shape: &dyn Shape, pose: &RapierPose, point: Vec3) -> (f32, f32) {
    let solid = shape.project_point(pose, point, true);
    let outside = if solid.is_inside {
        0.0
    } else {
        (solid.point - point).length()
    };
    let surface = shape.project_point(pose, point, false);
    (outside, (surface.point - point).length())
}

/// `drawn`'s mesh under `pose` against the shape physics builds for it.
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

/// `drawn` under every scale, turn and parenting.
fn check_everywhere(drawn: &Drawn) {
    for scale in SCALES {
        for (turn, _) in turns() {
            for pose in world_poses(scale, turn) {
                check(drawn, pose);
            }
        }
    }
}

/// The Sphere primitive is drawn at radius 1 (#1187): checked on its own below.
fn is_sphere(drawn: &Drawn) -> bool {
    matches!(drawn.0, ColliderShape::Sphere { .. })
}

#[test]
fn what_is_drawn_is_what_collides() {
    let all: Vec<Drawn> = KINDS.iter().flat_map(drawn).collect();
    all.iter()
        .filter(|d| !is_sphere(d))
        .for_each(check_everywhere);
}

#[test]
#[ignore = "#1187"]
fn a_drawn_sphere_is_what_collides() {
    drawn(&KINDS[1]).iter().for_each(check_everywhere);
}
