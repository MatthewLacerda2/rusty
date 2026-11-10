//! The parallax projection (#244, #1995), no GPU: against the box and the sphere,
//! continuous across a face, turned with an oriented box, and unchanged for a
//! world-aligned one.

use glam::{EulerRot, Quat, Vec3};

use super::*;

#[test]
fn parallax_corrects_toward_box_wall() {
    // A fragment at the centre of a [-2, 2] box reflecting +X hits (2, 0, 0).
    let probe = ReflectionProbe::new(Vec3::ZERO, Vec3::splat(2.0));
    let corrected = probe.parallax_correct(Vec3::ZERO, Vec3::X);
    assert!((corrected - Vec3::X).length() < 1e-5, "got {corrected:?}");
}

#[test]
fn parallax_offcentre_fragment_bends_direction() {
    // Off the centre, +X hits the wall at (2, 1, 0): the room-tracking bend.
    let probe = ReflectionProbe::new(Vec3::ZERO, Vec3::splat(2.0));
    let corrected = probe.parallax_correct(Vec3::new(-1.0, 1.0, 0.0), Vec3::X);
    let expected = Vec3::new(2.0, 1.0, 0.0).normalize();
    assert!((corrected - expected).length() < 1e-4, "got {corrected:?}");
}

#[test]
fn a_fragment_outside_the_box_projects_continuously() {
    // #1995: past a face the ray still leaves through the box (or is held at the
    // fragment), so just outside reflects what just inside does: no step.
    let probe = ReflectionProbe::new(Vec3::ZERO, Vec3::splat(2.0));
    for raw in [
        Vec3::new(1.0, 1.0, 0.0),
        Vec3::new(-1.0, 0.5, 0.3),
        Vec3::NEG_X,
    ] {
        let inside = probe.parallax_correct(Vec3::new(1.999, -1.5, 0.5), raw);
        let outside = probe.parallax_correct(Vec3::new(2.001, -1.5, 0.5), raw);
        assert!(
            (inside - outside).length() < 2e-3,
            "{raw}: {inside} / {outside}"
        );
        assert!(outside.is_finite() && (outside.length() - 1.0).abs() < 1e-5);
    }
    // Outside, a ray turning parallel to the face it is past changes smoothly: a
    // hit held at the fragment one way and at the far wall the other was a line.
    let past = Vec3::new(2.5, -1.5, 0.5);
    let tilt = |x: f32| probe.parallax_correct(past, Vec3::new(x, 1.0, -0.4));
    assert!(
        (tilt(1e-4) - tilt(-1e-4)).length() < 1e-3,
        "{} / {}",
        tilt(1e-4),
        tilt(-1e-4)
    );
}

#[test]
fn parallax_on_a_box_face_with_an_axis_aligned_ray_stays_finite() {
    // A fragment on the floor reflecting straight up exits through the ceiling.
    let probe = ReflectionProbe::new(Vec3::ZERO, Vec3::splat(2.0));
    let floor = Vec3::new(1.0, -2.0, 0.0);
    let out = probe.parallax_correct(floor, Vec3::Y);
    assert!((out - Vec3::new(1.0, 2.0, 0.0).normalize()).length() < 1e-5);
    // Leaving through the face it sits on: held at the fragment itself.
    let down = probe.parallax_correct(floor, Vec3::NEG_Y);
    assert!((down - floor.normalize()).length() < 1e-5, "{down}");
}

#[test]
fn a_rotated_box_projects_as_its_unrotated_twin_rotated() {
    let mut flat = ReflectionProbe::new(Vec3::new(1.0, 0.5, -0.5), Vec3::ONE);
    (flat.box_min, flat.box_max) = (Vec3::new(-4.0, -1.0, -2.0), Vec3::new(3.0, 2.0, 1.0));
    let q = Quat::from_euler(EulerRot::YXZ, 0.65, 0.17, -0.3);
    let turned = ReflectionProbe {
        rotation: q,
        ..flat.clone()
    };
    let c = flat.position;
    for (p, r) in samples(48) {
        let p = flat.box_min + (flat.box_max - flat.box_min) * p;
        let want = q * flat.parallax_correct(p, r);
        let got = turned.parallax_correct(c + q * (p - c), q * r);
        assert!((got - want).length() < 1e-4, "{p} {r}: {got} vs {want}");
    }
}

#[test]
fn a_world_aligned_box_projects_as_before_inside_it() {
    // #1995's frame adds exact zeros for an identity rotation: every fragment inside
    // the box gets the pre-#1995 projection, compared with `==`.
    let mut probe = ReflectionProbe::new(Vec3::new(0.3, 1.1, -0.7), Vec3::ONE);
    (probe.box_min, probe.box_max) = (Vec3::new(-3.0, 0.0, -5.0), Vec3::new(4.0, 3.0, 2.0));
    for (p, r) in samples(256) {
        let p = probe.box_min + (probe.box_max - probe.box_min) * p;
        assert_eq!(
            probe.parallax_correct(p, r),
            before_1995(&probe, p, r),
            "{p} {r}"
        );
    }
}

#[test]
fn a_sphere_projects_against_its_surface() {
    let mut probe = ReflectionProbe::new(Vec3::ZERO, Vec3::ONE);
    (probe.shape, probe.radius) = (ProbeShape::Sphere, 2.0);
    let hit = probe.parallax_correct(Vec3::new(0.0, 1.0, 0.0), Vec3::X);
    let expected = Vec3::new(3f32.sqrt(), 1.0, 0.0).normalize();
    assert!((hit - expected).length() < 1e-5, "{hit}");
    let raw = Vec3::new(-1.0, 0.4, 0.2);
    let inside = probe.parallax_correct(Vec3::new(0.0, 0.0, 1.9999), raw);
    let outside = probe.parallax_correct(Vec3::new(0.0, 0.0, 2.0001), raw);
    assert!((inside - outside).length() < 2e-3, "{inside} / {outside}");
}

/// `n` (point in the unit cube, unit direction) pairs from a fixed LCG.
fn samples(n: usize) -> Vec<(Vec3, Vec3)> {
    let mut s = 0x2545_f491_u32;
    let mut next = move || {
        s = s.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (s >> 8) as f32 / (1u32 << 24) as f32
    };
    (0..n)
        .map(|_| {
            let p = Vec3::new(next(), next(), next());
            let r = Vec3::new(next(), next(), next()) * 2.0 - Vec3::ONE;
            (p, r.normalize_or(Vec3::Y))
        })
        .collect()
}

/// The projection as it ran before #1995, for a fragment inside the box.
fn before_1995(probe: &ReflectionProbe, world_pos: Vec3, reflection: Vec3) -> Vec3 {
    let dir = reflection.normalize_or_zero();
    let mut t = NO_HIT;
    for axis in 0..3 {
        if dir[axis].abs() > PARALLEL_EPSILON {
            let first = (probe.box_max[axis] - world_pos[axis]) / dir[axis];
            let second = (probe.box_min[axis] - world_pos[axis]) / dir[axis];
            t = t.min(first.max(second));
        }
    }
    if t <= 0.0 || t >= NO_HIT {
        return dir;
    }
    (world_pos + dir * t - probe.position).normalize()
}
