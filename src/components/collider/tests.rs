//! src/components/collider/tests.rs — the collider's world AABB and serde.

use std::f32::consts::FRAC_PI_2;

use glam::Quat;

use super::*;

/// A pose at `pos`, turned by `rot`, scaled by `scale`.
fn at(pos: Vec3, rot: Quat, scale: Vec3) -> Pose {
    Pose { pos, rot, scale }
}

fn moved(pos: Vec3) -> Pose {
    at(pos, Quat::IDENTITY, Vec3::ONE)
}

#[test]
fn mesh_collider_world_aabb_follows_transform() {
    let c = ColliderComponent::from_mesh_bounds(false, Vec3::splat(-1.0), Vec3::splat(1.0));
    assert!(matches!(c.shape, ColliderShape::Mesh { convex: false, .. }));

    // Translate +Y by 5 and scale x2: the local [-1,1] box maps to [-2,2]±offset.
    let world = at(Vec3::new(0.0, 5.0, 0.0), Quat::IDENTITY, Vec3::splat(2.0));
    let (min, max) = c.calculate_world_aabb(world);
    assert!((min - Vec3::new(-2.0, 3.0, -2.0)).length() < 1e-4);
    assert!((max - Vec3::new(2.0, 7.0, 2.0)).length() < 1e-4);
}

fn capsule(radius: f32, height: f32, axis: CapsuleAxis) -> ColliderComponent {
    ColliderComponent {
        shape: ColliderShape::Capsule {
            radius,
            height,
            axis,
        },
        ..ColliderComponent::from_mesh_bounds(false, Vec3::ZERO, Vec3::ZERO)
    }
}

fn assert_vec(got: Vec3, want: Vec3) {
    assert!((got - want).length() < 1e-4, "got {got}, want {want}");
}

#[test]
fn capsule_world_aabb_spans_full_height_along_its_axis() {
    // Unity semantics: height 2 is the full end-to-end length, caps included.
    let c = capsule(0.5, 2.0, CapsuleAxis::Y);
    let (min, max) = c.calculate_world_aabb(moved(Vec3::new(1.0, 3.0, 0.0)));
    assert_vec(min, Vec3::new(0.5, 2.0, -0.5));
    assert_vec(max, Vec3::new(1.5, 4.0, 0.5));

    let (min, max) = capsule(0.25, 3.0, CapsuleAxis::X).calculate_world_aabb(moved(Vec3::ZERO));
    assert_vec(min, Vec3::new(-1.5, -0.25, -0.25));
    assert_vec(max, Vec3::new(1.5, 0.25, 0.25));

    let (min, max) = capsule(0.25, 3.0, CapsuleAxis::Z).calculate_world_aabb(moved(Vec3::ZERO));
    assert_vec(min, Vec3::new(-0.25, -0.25, -1.5));
    assert_vec(max, Vec3::new(0.25, 0.25, 1.5));
}

#[test]
fn short_capsule_bounds_are_its_sphere() {
    // height < 2r: the caps meet, the capsule is a sphere of radius r.
    let (min, max) = capsule(1.0, 0.5, CapsuleAxis::Y).calculate_world_aabb(moved(Vec3::ZERO));
    assert_vec(min, Vec3::splat(-1.0));
    assert_vec(max, Vec3::splat(1.0));
}

#[test]
fn rotated_capsule_bounds_follow_the_rotation() {
    // A Y capsule turned 90° about Z lies along X.
    let rot = at(Vec3::ZERO, Quat::from_rotation_z(FRAC_PI_2), Vec3::ONE);
    let (min, max) = capsule(0.5, 4.0, CapsuleAxis::Y).calculate_world_aabb(rot);
    assert_vec(min, Vec3::new(-2.0, -0.5, -0.5));
    assert_vec(max, Vec3::new(2.0, 0.5, 0.5));
}

#[test]
fn capsule_axis_names_round_trip() {
    for a in CapsuleAxis::ALL {
        assert_eq!(CapsuleAxis::parse(a.as_str()), Some(a));
    }
    assert_eq!(CapsuleAxis::parse("y"), None);
    assert_eq!(CapsuleAxis::default(), CapsuleAxis::Y);
}

#[test]
fn pre_447_collider_json_loads_with_default_material() {
    let json = r#"{"active":true,"shape":{"Sphere":{"radius":1.0}},"is_trigger":false,
        "aabb_min":[0,0,0],"aabb_max":[0,0,0]}"#;
    let c: ColliderComponent = serde_json::from_str(json).unwrap();
    assert_eq!(c.material, PhysicsMaterial::default());
}

fn cylinder(radius: f32, height: f32) -> ColliderComponent {
    ColliderComponent {
        shape: ColliderShape::Cylinder { radius, height },
        ..ColliderComponent::from_mesh_bounds(false, Vec3::ZERO, Vec3::ZERO)
    }
}

#[test]
fn axis_aligned_cylinder_bounds_are_unchanged() {
    let (min, max) = cylinder(0.5, 2.0).calculate_world_aabb(moved(Vec3::new(1.0, 2.0, 3.0)));
    assert_vec(min, Vec3::new(0.5, 1.0, 2.5));
    assert_vec(max, Vec3::new(1.5, 3.0, 3.5));
}

#[test]
fn stretched_cylinder_and_capsule_bounds_stay_round() {
    // #1140: physics sizes the radius by the larger cross scale, so the box does
    // too — scale z 3 widens x as well (it used to say x = ±0.5).
    let s = at(Vec3::ZERO, Quat::IDENTITY, Vec3::new(1.0, 1.0, 3.0));
    let (min, max) = cylinder(0.5, 2.0).calculate_world_aabb(s);
    assert_vec(min, Vec3::new(-1.5, -1.0, -1.5));
    assert_vec(max, Vec3::new(1.5, 1.0, 1.5));
    // The capsule's segment keeps its drawn half-length 1.5 (#1191), so its
    // wider caps stick out past the drawn ±2.
    let (min, max) = capsule(0.5, 4.0, CapsuleAxis::Y).calculate_world_aabb(s);
    assert_vec(min, Vec3::new(-1.5, -3.0, -1.5));
    assert_vec(max, Vec3::new(1.5, 3.0, 1.5));
}

#[test]
fn round_radius_takes_the_larger_cross_scale_and_ignores_mirrors() {
    let s = Vec3::new(-2.0, 3.0, 4.0);
    assert_eq!(round_radius(0.5, s, CapsuleAxis::X), 2.0);
    assert_eq!(round_radius(0.5, s, CapsuleAxis::Y), 2.0);
    assert_eq!(round_radius(0.5, s, CapsuleAxis::Z), 1.5);
}

#[test]
fn a_flattened_capsule_keeps_its_rotation() {
    // A zero scale used to lose the rotation; the pose carries it, so a flat Y
    // capsule turned 90° about Z still lies along X.
    let flat = at(
        Vec3::ZERO,
        Quat::from_rotation_z(FRAC_PI_2),
        Vec3::new(0.0, 1.0, 1.0),
    );
    let (min, max) = capsule(0.5, 2.0, CapsuleAxis::Y).calculate_world_aabb(flat);
    assert_vec(min, Vec3::new(-1.0, -0.5, -0.5));
    assert_vec(max, Vec3::new(1.0, 0.5, 0.5));
}
