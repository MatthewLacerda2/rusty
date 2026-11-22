use glam::{Quat, Vec3};

use super::super::{ProbeShape, ReflectionProbe, ReflectionProbeSet};
use super::ProbeBlend;

fn mixed() -> ReflectionProbeSet {
    let mut set = ReflectionProbeSet::new();
    set.add(Vec3::ZERO, Vec3::new(3.0, 2.0, 4.0));
    let turned = set.add(Vec3::new(4.0, 0.5, -1.0), Vec3::new(2.5, 1.5, 1.0));
    set.edit(turned, |p| {
        p.rotation = Quat::from_euler(glam::EulerRot::YXZ, 0.7, 0.2, -0.4);
        p.blend_distance = 2.0;
    });
    let sphere = set.add(Vec3::new(-3.0, 1.0, 2.0), Vec3::ONE);
    set.edit(sphere, |p| {
        (p.shape, p.radius, p.importance) = (ProbeShape::Sphere, 2.5, 1);
        p.rotation = Quat::from_rotation_z(1.1);
    });
    let hard = set.add(Vec3::new(1.0, -1.0, 1.0), Vec3::splat(1.5));
    set.edit(hard, |p| p.blend_distance = 0.0);
    let important = set.add(Vec3::new(2.0, 0.0, 2.0), Vec3::splat(2.0));
    set.edit(important, |p| (p.importance, p.blend_distance) = (2, 3.0));
    set.add(Vec3::new(900.0, 3.0, -700.0), Vec3::splat(6.0));
    set
}

/// Compared by bits: `-0.0` and `0.0` differ here.
fn bits(b: Option<ProbeBlend>) -> Option<(usize, u32, Option<usize>, u32)> {
    b.map(|b| {
        (
            b.first,
            b.first_weight.to_bits(),
            b.second,
            b.second_weight.to_bits(),
        )
    })
}

fn agrees_over(set: &ReflectionProbeSet, centre: Vec3, half: f32, steps: i32) {
    let prepared = set.prepared();
    let mut reached = 0;
    for x in -steps..=steps {
        for y in -steps..=steps {
            for z in -steps..=steps {
                let point = centre + Vec3::new(x as f32, y as f32, z as f32) * half / steps as f32;
                let (want, got) = (set.blend(point), prepared.blend(point));
                assert_eq!(bits(got), bits(want), "at {point}");
                reached += usize::from(want.is_some());
            }
        }
    }
    assert!(reached > 0, "the grid reaches some probe");
}

#[test]
fn the_prepared_blend_equals_the_blend_bit_for_bit() {
    let set = mixed();
    agrees_over(&set, Vec3::ZERO, 9.0, 24);
    agrees_over(&set, Vec3::new(900.0, 3.0, -700.0), 9.0, 10);
}

#[test]
fn the_edge_of_the_blend_distance_agrees() {
    let set = mixed();
    let prepared = set.prepared();
    for k in -64..=64 {
        let past = 3.0 + 1.0 + k as f32 * 1e-6;
        for point in [
            Vec3::new(past, 0.0, 0.0),
            Vec3::new(0.0, 0.0, -4.0 - 1.0 - k as f32 * 1e-6),
        ] {
            assert_eq!(
                bits(prepared.blend(point)),
                bits(set.blend(point)),
                "at {point}"
            );
        }
    }
}

#[test]
fn a_nan_point_and_an_empty_set_agree() {
    let set = mixed();
    let nan = Vec3::new(f32::NAN, 0.0, 0.0);
    assert_eq!(bits(set.prepared().blend(nan)), bits(set.blend(nan)));
    assert_eq!(ReflectionProbeSet::new().prepared().blend(Vec3::ZERO), None);
}

#[test]
fn an_update_follows_the_set_it_is_given() {
    let mut set = mixed();
    let mut prepared = set.prepared();
    prepared.update(&set.probes);
    agrees_over(&set, Vec3::ZERO, 9.0, 12);
    set.edit(1, |p| p.rotation = Quat::from_rotation_x(0.3));
    set.move_probe(0, Vec3::new(1.0, 0.0, 0.0));
    prepared.update(&set.probes);
    agrees_over(&set, Vec3::ZERO, 9.0, 12);
    set.probes.truncate(2);
    prepared.update(&set.probes);
    assert_eq!(prepared.probes(), &set.probes[..]);
    agrees_over(&set, Vec3::ZERO, 9.0, 12);
}

#[test]
fn a_nearly_unit_rotation_agrees_past_the_corners() {
    let mut set = ReflectionProbeSet::new();
    set.add(Vec3::ZERO, Vec3::splat(10.0));
    let q = Quat::from_euler(glam::EulerRot::YXZ, 0.9, 0.3, 0.5) * (1.0 - 0.9e-4);
    // Written straight to the field (`edit` would normalise it): unnormalised, the frame's
    // turn and glam's disagree by millimetres past the corners.
    (set.probes[0].rotation, set.probes[0].blend_distance) = (q, 2.0);
    let (probe, prepared): (&ReflectionProbe, _) = (&set.probes[0], set.prepared());
    let frame = probe.frame();
    let mut reached = 0;
    for corner in 0..8 {
        let bit = |b: usize| Vec3::splat(f32::from(u8::from(corner & b != 0)));
        let mask = bit(1) * Vec3::X + bit(2) * Vec3::Y + bit(4) * Vec3::Z;
        let local = frame.box_min + (frame.box_max - frame.box_min) * mask;
        let mut at = q.normalize() * local;
        for _ in 0..50 {
            at += q.normalize() * (local - frame.to_frame(at));
        }
        for dir in [Vec3::X, Vec3::Y, Vec3::Z, -Vec3::X, -Vec3::Y, -Vec3::Z] {
            for k in 19_800..20_200 {
                let point = at + dir * (k as f32 * 1e-4);
                let want = set.blend(point);
                assert_eq!(bits(prepared.blend(point)), bits(want), "at {point}");
                reached += usize::from(want.is_some());
            }
        }
    }
    assert!(reached > 0, "the sweep crosses the edge");
}
