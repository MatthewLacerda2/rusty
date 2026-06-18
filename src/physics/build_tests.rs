//! src/physics/build_tests.rs — unit tests for body/collider construction.
//!
//! Kept in a sibling module so `build.rs` stays under the size cap. These assert the
//! *values* `build.rs` computes (baked extents, layer bits, body class) rather than
//! just that a collider is produced, so arithmetic/comparison mutations are caught.

use glam::Vec3;
use rapier3d::prelude::*;

use super::build::{build_shape, collider_inputs, interaction_groups, is_kinematic};
use crate::components::{ColliderComponent, ColliderShape, Entity, RigidBodyComponent};

/// Local-AABB half-extents `(x, y, z)` of a built collider.
fn half_extents(c: &Collider) -> [f32; 3] {
    let aabb = c.shape().compute_local_aabb();
    [aabb.maxs[0], aabb.maxs[1], aabb.maxs[2]]
}

#[test]
fn box_bakes_size_and_scale_into_half_extents() {
    // half = size * scale * 0.5, per axis. Asymmetric values so a `*`->`+`/`/`
    // mutation in any factor changes the result.
    let c = build_shape(
        &ColliderShape::Box {
            size: Vec3::new(4.0, 6.0, 8.0),
        },
        Vec3::splat(2.0),
        None,
    )
    .unwrap();
    let h = half_extents(&c);
    assert!((h[0] - 4.0).abs() < 1e-4, "x half-extent {h:?}"); // 4*2*0.5
    assert!((h[1] - 6.0).abs() < 1e-4, "y half-extent {h:?}"); // 6*2*0.5
    assert!((h[2] - 8.0).abs() < 1e-4, "z half-extent {h:?}"); // 8*2*0.5
}

#[test]
fn sphere_bakes_radius_times_max_scale() {
    // r = radius * scale.max_element() = 2 * 3 = 6.
    let c = build_shape(
        &ColliderShape::Sphere { radius: 2.0 },
        Vec3::new(3.0, 1.0, 1.0),
        None,
    )
    .unwrap();
    for axis in half_extents(&c) {
        assert!((axis - 6.0).abs() < 1e-4, "sphere radius");
    }
}

#[test]
fn cylinder_bakes_radius_x_and_half_height_y() {
    // r = radius * scale.x = 2*3 = 6; half_h = height * scale.y * 0.5 = 10*4*0.5 = 20.
    let c = build_shape(
        &ColliderShape::Cylinder {
            radius: 2.0,
            height: 10.0,
        },
        Vec3::new(3.0, 4.0, 1.0),
        None,
    )
    .unwrap();
    let h = half_extents(&c);
    assert!((h[0] - 6.0).abs() < 1e-4, "cylinder radius x {h:?}");
    assert!((h[1] - 20.0).abs() < 1e-4, "cylinder half-height y {h:?}");
    assert!((h[2] - 6.0).abs() < 1e-4, "cylinder radius z {h:?}");
}

#[test]
fn mesh_collider_needs_at_least_four_points() {
    // Exactly four non-coplanar points form a valid convex hull (a tetrahedron); the
    // `< 4` guard must admit four, so a `<=`-mutation that rejects them is caught.
    let tetra = vec![
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0],
    ];
    let shape = ColliderShape::Mesh {
        convex: true,
        local_min: Vec3::ZERO,
        local_max: Vec3::ONE,
    };
    assert!(build_shape(&shape, Vec3::ONE, Some((&tetra, &[]))).is_some());
}

#[test]
fn interaction_groups_encode_layer_bit_and_filter() {
    let g = interaction_groups(5, 0xFF);
    assert_eq!(g.memberships, Group::from_bits_truncate(1 << 5));
    assert_eq!(g.filter, Group::from_bits_truncate(0xFF));
    // Layers >= 32 have no representable bit -> empty membership, never a wrapped
    // (panicking) `1 << 32` shift.
    assert_eq!(interaction_groups(32, 0xFF).memberships, Group::empty());
}

#[test]
fn is_kinematic_only_for_non_static_bodies_without_a_dynamic_rigidbody() {
    let dynamic = RigidBodyComponent {
        active: true,
        is_kinematic: false,
        mass: 1.0,
        velocity: Vec3::ZERO,
        use_gravity: true,
    };
    assert!(!is_kinematic(true, None), "static is not kinematic");
    assert!(
        is_kinematic(false, None),
        "no rigidbody defaults to kinematic"
    );
    assert!(
        !is_kinematic(false, Some(&dynamic)),
        "dynamic is not kinematic"
    );
}

#[test]
fn collider_inputs_respects_the_active_flag() {
    let mut e = Entity::new(1, "Box".to_string());
    e.collider = Some(ColliderComponent {
        active: false,
        shape: ColliderShape::Box { size: Vec3::ONE },
        is_trigger: false,
        aabb_min: Vec3::ZERO,
        aabb_max: Vec3::ZERO,
    });
    assert!(
        collider_inputs(&e).is_none(),
        "an inactive collider yields no body"
    );
    // Activating it produces inputs — pins the match guard in both directions.
    e.collider.as_mut().unwrap().active = true;
    assert!(
        collider_inputs(&e).is_some(),
        "an active collider yields inputs"
    );
}
