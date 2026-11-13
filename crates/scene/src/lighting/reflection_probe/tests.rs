//! Unit tests for the reflection-probe data model and its settings (no GPU); the
//! parallax math is `projection_tests.rs`, the volume `volume_tests.rs`.

use super::*;

#[test]
fn add_move_set_remove() {
    let mut set = ReflectionProbeSet::new();
    let i = set.add(Vec3::ZERO, Vec3::splat(2.0));
    assert_eq!(set.len(), 1);
    assert_eq!(set.probes[i].box_min, Vec3::splat(-2.0));
    assert_eq!(set.probes[i].box_max, Vec3::splat(2.0));

    set.move_probe(i, Vec3::new(10.0, 0.0, 0.0));
    assert_eq!(set.probes[i].position, Vec3::new(10.0, 0.0, 0.0));
    // Box stays centred on the new position with the same half-extents.
    assert_eq!(set.probes[i].box_min, Vec3::new(8.0, -2.0, -2.0));
    assert_eq!(set.probes[i].box_max, Vec3::new(12.0, 2.0, 2.0));

    set.set_box(i, Vec3::new(5.0, 5.0, 5.0), Vec3::new(1.0, 1.0, 1.0));
    // Corners are normalized so min <= max.
    assert_eq!(set.probes[i].box_min, Vec3::ONE);
    assert_eq!(set.probes[i].box_max, Vec3::splat(5.0));

    set.set_cubemap(i, "probe0.ktx2".to_string());
    assert_eq!(set.probes[i].cubemap_path, "probe0.ktx2");

    set.remove(i);
    assert!(set.is_empty());
}

#[test]
fn contains_inclusive() {
    let probe = ReflectionProbe::new(Vec3::ZERO, Vec3::ONE);
    assert!(probe.contains(Vec3::ZERO));
    assert!(probe.contains(Vec3::ONE)); // boundary is inclusive
    assert!(!probe.contains(Vec3::new(1.001, 0.0, 0.0)));
}

#[test]
fn set_round_trips_through_json() {
    let mut set = ReflectionProbeSet::new();
    let i = set.add(Vec3::new(1.0, 2.0, 3.0), Vec3::splat(4.0));
    set.set_cubemap(i, "reflections/room.ktx2".to_string());
    let json = serde_json::to_string(&set).unwrap();
    let back: ReflectionProbeSet = serde_json::from_str(&json).unwrap();
    assert_eq!(back.probes, set.probes);
}

#[test]
fn box_distance_is_zero_inside_and_to_the_nearest_face_outside() {
    let probe = ReflectionProbe::new(Vec3::ZERO, Vec3::ONE);
    assert_eq!(probe.volume_distance_sq(Vec3::new(0.5, -0.9, 0.0)), 0.0);
    assert_eq!(probe.volume_distance_sq(Vec3::new(3.0, 0.0, 0.0)), 4.0);
    assert_eq!(probe.volume_distance_sq(Vec3::new(2.0, 2.0, 0.0)), 2.0);
}

#[test]
fn settings_clamp_to_their_ranges() {
    let mut set = ReflectionProbeSet::new();
    set.add(Vec3::ZERO, Vec3::ONE);
    assert!(set.edit(0, |p| {
        (p.resolution, p.blend_distance) = (Some(300), -2.0);
        (p.near_clip, p.far_clip, p.intensity) = (0.0, 0.0, f32::NAN);
        (p.box_min, p.box_max) = (Vec3::ONE, -Vec3::ONE);
    }));
    let p = &set.probes[0];
    assert_eq!((p.resolution, p.blend_distance), (Some(256), 0.0));
    assert_eq!((p.near_clip, p.far_clip), (0.01, 0.02));
    assert_eq!(p.intensity, 1.0);
    assert_eq!((p.box_min, p.box_max), (-Vec3::ONE, Vec3::ONE));
    assert!(!set.edit(5, |_| {}), "no probe 5");
    assert_eq!((clamp_resolution(1), clamp_resolution(9000)), (16, 2048));
    assert_eq!((clamp_resolution(400), clamp_resolution(512)), (512, 512));
}
