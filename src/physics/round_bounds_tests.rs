//! src/physics/round_bounds_tests.rs — a round collider's stored world AABB
//! against the rapier shape `build_shape` makes for it (#1140): the two read one
//! radius rule (`components::round_radius`), so the box always holds the shape.

use glam::{Mat4, Quat, Vec3};
use rapier3d::prelude::*;

use super::build::build_shape;
use crate::components::{CapsuleAxis, ColliderComponent, ColliderShape};

#[test]
fn round_shape_aabb_contains_its_rapier_shape_under_non_uniform_scale() {
    // #1140: the stored AABB is sized from the shape physics builds, so it holds it.
    let (scale, pos) = (Vec3::new(1.0, 2.0, 3.0), Vec3::new(4.0, -1.0, 2.0));
    for rot in [Quat::IDENTITY, Quat::from_rotation_y(0.7)] {
        for shape in [
            ColliderShape::Cylinder {
                radius: 0.5,
                height: 2.0,
            },
            ColliderShape::Capsule {
                radius: 0.5,
                height: 3.0,
                axis: CapsuleAxis::X,
            },
            ColliderShape::Sphere { radius: 0.5 },
        ] {
            let mut c = ColliderComponent::from_mesh_bounds(false, Vec3::ZERO, Vec3::ZERO);
            c.shape = shape;
            let (min, max) =
                c.calculate_world_aabb(Mat4::from_scale_rotation_translation(scale, rot, pos));
            let want = build_shape(&c.shape, scale, None)
                .unwrap()
                .shape()
                .compute_aabb(&Pose::from_parts(pos, rot));
            let (wmin, wmax) = (want.mins, want.maxs);
            let tol = if rot == Quat::IDENTITY {
                1e-4
            } else {
                f32::MAX
            };
            assert!(
                min.cmple(wmin + 1e-4).all() && max.cmpge(wmax - 1e-4).all(),
                "{:?}",
                c.shape
            );
            assert!(
                (min - wmin).abs().max_element() < tol && (max - wmax).abs().max_element() < tol
            );
        }
    }
}
