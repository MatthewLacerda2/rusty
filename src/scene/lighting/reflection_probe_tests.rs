//! Unit tests for reflection-probe selection + parallax math (no GPU).

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
fn select_nearest_containing_probe() {
    let mut set = ReflectionProbeSet::new();
    // Two overlapping boxes covering the origin; nearest capture point wins.
    set.add(Vec3::new(3.0, 0.0, 0.0), Vec3::splat(5.0)); // covers origin, dist 3
    set.add(Vec3::new(1.0, 0.0, 0.0), Vec3::splat(5.0)); // covers origin, dist 1 (nearer)
    let idx = set.select_index(Vec3::ZERO);
    assert_eq!(idx, Some(1));
}

#[test]
fn select_none_outside_all_boxes() {
    let mut set = ReflectionProbeSet::new();
    set.add(Vec3::ZERO, Vec3::ONE);
    assert!(set.select(Vec3::new(100.0, 0.0, 0.0)).is_none());
    assert!(set.select_index(Vec3::new(100.0, 0.0, 0.0)).is_none());
}

#[test]
fn parallax_corrects_toward_box_wall() {
    // Probe centred at origin, box spanning [-2,2]. A fragment at the box centre
    // reflecting straight +X must hit the +X wall at (2,0,0); the corrected direction
    // from the probe centre is then exactly +X.
    let probe = ReflectionProbe::new(Vec3::ZERO, Vec3::splat(2.0));
    let corrected = probe.parallax_correct(Vec3::ZERO, Vec3::X);
    assert!((corrected - Vec3::X).length() < 1e-5, "got {corrected:?}");
}

#[test]
fn parallax_offcentre_fragment_bends_direction() {
    // A fragment OFF the probe centre reflecting +X hits the +X wall at (2, y, 0);
    // the corrected dir from the centre is NOT pure +X — it bends toward that wall
    // point. This is the room-tracking behaviour the acceptance criterion asks for.
    let probe = ReflectionProbe::new(Vec3::ZERO, Vec3::splat(2.0));
    let frag = Vec3::new(-1.0, 1.0, 0.0);
    let corrected = probe.parallax_correct(frag, Vec3::X);
    // Hit point is (2, 1, 0); normalized that has a positive Y component.
    let expected = Vec3::new(2.0, 1.0, 0.0).normalize();
    assert!((corrected - expected).length() < 1e-4, "got {corrected:?}");
    // And it genuinely differs from the uncorrected +X direction.
    assert!((corrected - Vec3::X).length() > 1e-3);
}

#[test]
fn parallax_degenerate_returns_normalized_dir() {
    // Probe whose centre sits OUTSIDE its box (pathological) -> no forward hit; the
    // function returns the plain normalized reflection rather than NaN.
    let mut probe = ReflectionProbe::new(Vec3::ZERO, Vec3::ONE);
    probe.box_min = Vec3::new(10.0, 10.0, 10.0);
    probe.box_max = Vec3::new(12.0, 12.0, 12.0);
    let corrected = probe.parallax_correct(Vec3::ZERO, Vec3::new(0.0, 0.0, 2.0));
    assert!(corrected.is_finite());
    assert!((corrected.length() - 1.0).abs() < 1e-5);
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
