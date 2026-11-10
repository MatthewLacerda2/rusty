//! A probe's volume (#1995), no GPU: an oriented box and a sphere, what holds them
//! in range, and what the scene file writes for them.

use glam::{Quat, Vec3};

use super::*;

fn turned(half: Vec3, rotation: Quat) -> ReflectionProbe {
    ReflectionProbe {
        rotation,
        ..ReflectionProbe::new(Vec3::new(10.0, 0.0, 0.0), half)
    }
}

#[test]
fn an_oriented_box_contains_and_reaches_in_its_own_frame() {
    // A 4 x 1 x 1 half-extent box turned 90 degrees about Y lies along Z.
    let p = turned(
        Vec3::new(4.0, 1.0, 1.0),
        Quat::from_rotation_y(90f32.to_radians()),
    );
    assert!(p.contains(Vec3::new(10.0, 0.0, 3.5)));
    assert!(!p.contains(Vec3::new(13.5, 0.0, 0.0)));
    let d = p.volume_distance_sq(Vec3::new(13.0, 0.0, 0.0));
    assert!((d - 4.0).abs() < 1e-4, "{d}");
    // The blend's influence follows the same frame.
    assert_eq!(p.influence(Vec3::new(10.0, 0.0, -3.9)), 1.0);
    assert!((p.influence(Vec3::new(10.0, 0.0, 4.5)) - 0.5).abs() < 1e-4);
}

#[test]
fn an_oriented_box_has_turned_corners_and_world_bounds() {
    let p = turned(Vec3::ONE, Quat::from_rotation_y(45f32.to_radians()));
    let (lo, hi) = p.world_bounds();
    let s = 2f32.sqrt();
    assert!((lo - Vec3::new(10.0 - s, -1.0, -s)).length() < 1e-5, "{lo}");
    assert!((hi - Vec3::new(10.0 + s, 1.0, s)).length() < 1e-5, "{hi}");
    let flat = turned(Vec3::ONE, Quat::IDENTITY);
    assert_eq!(flat.world_bounds(), (flat.box_min, flat.box_max));
    assert_eq!(flat.corners()[0], flat.box_min);
    assert_eq!(flat.corners()[7], flat.box_max);
}

#[test]
fn a_sphere_contains_and_reaches_by_its_radius() {
    let mut p = turned(Vec3::ONE, Quat::IDENTITY);
    (p.shape, p.radius) = (ProbeShape::Sphere, 3.0);
    assert!(p.contains(Vec3::new(12.9, 0.0, 0.0)));
    assert!(!p.contains(Vec3::new(10.0, 3.1, 0.0)));
    let d = p.volume_distance_sq(Vec3::new(10.0, 0.0, -5.0));
    assert!((d - 4.0).abs() < 1e-4, "{d}");
    assert_eq!(
        p.world_bounds(),
        (Vec3::new(7.0, -3.0, -3.0), Vec3::new(13.0, 3.0, 3.0))
    );
}

#[test]
fn rotation_and_radius_are_held_to_their_ranges() {
    let mut set = ReflectionProbeSet::new();
    set.add(Vec3::ZERO, Vec3::ONE);
    set.edit(0, |p| {
        (p.rotation, p.radius) = (Quat::from_xyzw(0.0, 2.0, 0.0, 0.0), -1.0)
    });
    assert_eq!(set.probes[0].rotation, Quat::from_xyzw(0.0, 1.0, 0.0, 0.0));
    assert_eq!(set.probes[0].radius, 0.01);
    set.edit(0, |p| {
        (p.rotation, p.radius) = (Quat::from_xyzw(f32::NAN, 0.0, 0.0, 1.0), f32::NAN)
    });
    assert_eq!(set.probes[0].rotation, Quat::IDENTITY);
    assert_eq!(set.probes[0].radius, DEFAULT_RADIUS);
}

#[test]
fn a_world_aligned_box_probe_writes_no_new_keys_and_a_sphere_round_trips() {
    let flat = serde_json::to_string(&ReflectionProbe::default()).unwrap();
    for key in ["rotation", "shape", "radius"] {
        assert!(!flat.contains(key), "{flat}");
    }
    let mut p = turned(Vec3::ONE, Quat::from_rotation_x(0.4));
    (p.shape, p.radius) = (ProbeShape::Sphere, 2.5);
    let json = serde_json::to_string(&p).unwrap();
    assert!(json.contains("\"Sphere\""), "{json}");
    assert_eq!(serde_json::from_str::<ReflectionProbe>(&json).unwrap(), p);
}
