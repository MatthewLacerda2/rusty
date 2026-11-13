//! Tests for `ReflectionProbeSet::blend` (#1584): Unity's *Blend Probes*.

use glam::Vec3;

use super::super::ReflectionProbeSet;

/// Two rooms side by side along x, sharing the wall plane at x = 10: A over
/// [0, 10], B over [10, 20], each with the default 1 m blend distance.
fn two_rooms() -> ReflectionProbeSet {
    let mut set = ReflectionProbeSet::new();
    set.add(Vec3::new(5.0, 0.0, 0.0), Vec3::splat(5.0));
    set.add(Vec3::new(15.0, 0.0, 0.0), Vec3::splat(5.0));
    set
}

fn at(x: f32) -> Vec3 {
    Vec3::new(x, 0.0, 0.0)
}

#[test]
fn deep_in_one_room_reflects_only_it() {
    let b = two_rooms().blend(at(3.0)).unwrap();
    assert_eq!((b.first, b.first_weight, b.second), (0, 1.0, None));
}

#[test]
fn walking_through_the_doorway_fades_from_one_room_to_the_other() {
    let set = two_rooms();
    let share_of_b = |x: f32| {
        let b = set.blend(at(x)).unwrap();
        let w_b = match b.first {
            1 => b.first_weight,
            _ => b.second_weight,
        };
        let total = b.first_weight + b.second_weight;
        assert!(
            (total - 1.0).abs() < 1e-6,
            "inside a box nothing is sky: {b:?}"
        );
        w_b
    };
    // B's share climbs steadily from 0 a blend distance before the wall to 1 a blend
    // distance past it: a fade, never a step.
    let mut last = 0.0;
    for step in 0..=40 {
        let x = 8.5 + step as f32 * 0.075;
        let w = share_of_b(x);
        assert!(w >= last - 1e-6, "B's share fell at x = {x}");
        assert!(w - last < 0.2, "B's share jumped at x = {x}: {last} → {w}");
        last = w;
    }
    assert_eq!(share_of_b(8.9), 0.0);
    assert!((share_of_b(10.0) - 0.5).abs() < 1e-6);
    assert_eq!(share_of_b(11.1), 1.0);
}

#[test]
fn past_the_last_box_the_probe_fades_into_the_sky() {
    let set = two_rooms();
    let b = set.blend(at(20.5)).unwrap();
    assert_eq!((b.first, b.second), (1, None));
    assert!((b.first_weight - 0.5).abs() < 1e-6);
    assert!(set.blend(at(21.5)).is_none(), "out of reach: the sky");
}

#[test]
fn a_zero_blend_distance_is_a_hard_edge() {
    let mut set = two_rooms();
    set.probes[0].blend_distance = 0.0;
    let b = set.blend(at(10.5)).unwrap();
    assert_eq!((b.first, b.first_weight, b.second), (1, 1.0, None));
}

#[test]
fn a_higher_importance_box_is_drawn_over_the_room_around_it() {
    let mut set = ReflectionProbeSet::new();
    set.add(Vec3::ZERO, Vec3::splat(20.0)); // the hall
    let alcove = set.add(Vec3::new(5.0, 0.0, 0.0), Vec3::splat(2.0));
    set.probes[alcove].importance = 2;
    // Inside the alcove it wins outright, though both boxes hold the point.
    let inside = set.blend(at(5.0)).unwrap();
    assert_eq!((inside.first, inside.first_weight), (alcove, 1.0));
    assert_eq!(inside.second_weight, 0.0);
    // Half a blend distance out of it, the hall fills what the alcove leaves.
    let leaving = set.blend(at(7.5)).unwrap();
    assert_eq!((leaving.first, leaving.second), (alcove, Some(0)));
    assert!((leaving.first_weight - 0.5).abs() < 1e-6);
    assert!((leaving.second_weight - 0.5).abs() < 1e-6);
}

#[test]
fn equal_candidates_rank_by_the_nearer_capture_then_the_index() {
    let mut set = ReflectionProbeSet::new();
    set.add(Vec3::new(3.0, 0.0, 0.0), Vec3::splat(5.0));
    set.add(Vec3::new(1.0, 0.0, 0.0), Vec3::splat(5.0));
    let b = set.blend(Vec3::ZERO).unwrap();
    assert_eq!((b.first, b.second), (1, Some(0)));
    set.add(Vec3::new(1.0, 0.0, 0.0), Vec3::splat(5.0));
    assert_eq!(
        set.blend(Vec3::ZERO).unwrap().first,
        1,
        "lower index on a tie"
    );
}
