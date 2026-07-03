//! src/physics/build_tests.rs — the single test home for `build.rs`.
//!
//! All unit tests for body/collider construction live here, in one place, rather
//! than split between an inline `#[cfg(test)] mod tests` in `build.rs` and this
//! sibling. The lint counts `build.rs` and this sibling as one logical unit (see
//! `tools/lint`), so there is no size-cap reason to test `build.rs` from two
//! locations. These assert the *values* `build.rs` computes (baked extents, layer
//! bits, body class) rather than just that a collider is produced, so
//! arithmetic/comparison mutations are caught.

use glam::Vec3;
use rapier3d::prelude::*;

use super::build::{build_shape, collider_inputs, interaction_groups, is_kinematic};
use crate::components::{
    ColliderComponent, ColliderShape, CollisionDetection, Entity, RigidBodyComponent,
};

/// Local-AABB half-extents `(x, y, z)` of a built collider.
fn half_extents(c: &Collider) -> [f32; 3] {
    let aabb = c.shape().compute_local_aabb();
    [aabb.maxs[0], aabb.maxs[1], aabb.maxs[2]]
}

/// A unit cube (half-extent 0.5) as rest-pose positions + triangle indices.
fn unit_cube() -> (Vec<[f32; 3]>, Vec<u32>) {
    let p = vec![
        [-0.5, -0.5, -0.5],
        [0.5, -0.5, -0.5],
        [0.5, 0.5, -0.5],
        [-0.5, 0.5, -0.5],
        [-0.5, -0.5, 0.5],
        [0.5, -0.5, 0.5],
        [0.5, 0.5, 0.5],
        [-0.5, 0.5, 0.5],
    ];
    #[rustfmt::skip]
    let i = vec![
        0, 1, 2, 0, 2, 3, 4, 6, 5, 4, 7, 6, 0, 4, 5,
        0, 5, 1, 3, 2, 6, 3, 6, 7, 0, 3, 7, 0, 7, 4, 1, 5, 6, 1, 6, 2,
    ];
    (p, i)
}

fn mesh_shape(convex: bool) -> ColliderShape {
    ColliderShape::Mesh {
        convex,
        local_min: Vec3::splat(-0.5),
        local_max: Vec3::splat(0.5),
    }
}

fn assert_cube_extent(collider: &Collider, half: f32) {
    let aabb = collider.shape().compute_local_aabb();
    for axis in 0..3 {
        assert!((aabb.maxs[axis] - half).abs() < 1e-4, "max {:?}", aabb.maxs);
        assert!((aabb.mins[axis] + half).abs() < 1e-4, "min {:?}", aabb.mins);
    }
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
fn convex_hull_from_mesh_matches_extents() {
    let (p, i) = unit_cube();
    let c = build_shape(&mesh_shape(true), Vec3::ONE, Some((&p, &i))).unwrap();
    assert_cube_extent(&c, 0.5);
}

#[test]
fn trimesh_from_mesh_matches_extents() {
    let (p, i) = unit_cube();
    let c = build_shape(&mesh_shape(false), Vec3::ONE, Some((&p, &i))).unwrap();
    assert_cube_extent(&c, 0.5);
}

#[test]
fn scale_is_baked_into_mesh_collider() {
    let (p, i) = unit_cube();
    let c = build_shape(&mesh_shape(false), Vec3::splat(2.0), Some((&p, &i))).unwrap();
    assert_cube_extent(&c, 1.0);
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
    assert!(build_shape(&mesh_shape(true), Vec3::ONE, Some((&tetra, &[]))).is_some());
}

#[test]
fn degenerate_mesh_yields_no_collider() {
    // Too few points for any shape.
    let sparse = vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
    assert!(build_shape(&mesh_shape(true), Vec3::ONE, Some((&sparse, &[0, 1, 2]))).is_none());
    // Enough points, but a trimesh needs triangles.
    let (p, _) = unit_cube();
    assert!(build_shape(&mesh_shape(false), Vec3::ONE, Some((&p, &[]))).is_none());
    // No geometry supplied at all.
    assert!(build_shape(&mesh_shape(true), Vec3::ONE, None).is_none());
}

#[test]
fn analytic_shapes_still_build_without_mesh() {
    assert!(build_shape(&ColliderShape::Sphere { radius: 1.0 }, Vec3::ONE, None).is_some());
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
        angular_velocity: Vec3::ZERO,
        use_gravity: true,
        collision_detection: CollisionDetection::Discrete,
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
