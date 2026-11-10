//! Projection far past a probe's volume (#1995): it fades back to infinite
//! projection past the influence volume, smoothly, so a large mesh's far fragments
//! keep a reflection that follows R instead of one patch of the cube.

use glam::{Quat, Vec3};

use super::*;

fn directions() -> [Vec3; 4] {
    [
        Vec3::new(1.0, 1.0, 0.0).normalize(),
        Vec3::new(-0.3, 0.8, 0.5).normalize(),
        Vec3::new(0.2, 0.9, -0.6).normalize(),
        Vec3::NEG_X,
    ]
}

#[test]
fn tens_of_metres_past_a_box_the_reflection_follows_r() {
    // The review's case: a [-4, 4] box, a floor fragment at x = 60.
    let probe = ReflectionProbe::new(Vec3::ZERO, Vec3::splat(4.0));
    let far = Vec3::new(60.0, -4.0, 3.0);
    for r in directions() {
        let got = probe.parallax_correct(far, r);
        assert!((got - r).length() < 1e-6, "{r}: {got}");
    }
    // Turned and as a sphere, too.
    let turned = ReflectionProbe {
        rotation: Quat::from_rotation_y(0.6),
        ..probe.clone()
    };
    let sphere = ReflectionProbe {
        shape: ProbeShape::Sphere,
        radius: 6.0,
        ..probe.clone()
    };
    for p in [turned, sphere] {
        for r in directions() {
            assert!((p.parallax_correct(far, r) - r).length() < 1e-6);
        }
    }
}

#[test]
fn the_fade_starts_past_the_influence_volume_and_is_continuous() {
    let probe = ReflectionProbe::new(Vec3::ZERO, Vec3::splat(4.0));
    let span = probe.frame().fade_span();
    assert!((span - 48f32.sqrt()).abs() < 1e-5, "{span}");
    let at = |x: f32, r: Vec3| probe.parallax_correct(Vec3::new(x, -2.0, 1.0), r);
    let start = 4.0 + probe.blend_distance;
    for r in directions() {
        // Within the influence volume: the plain projection, unfaded.
        let inside = at(start - 0.01, r);
        let frame = probe.frame();
        let plain = super::projection::project_in_volume(&frame, Vec3::new(4.99, -2.0, 1.0), r);
        assert!((inside - plain).length() < 1e-6, "{r}");
        // No step at either end of the fade, nor anywhere along it: a centimetre
        // turns the direction by at most the parallax's own rate (a step was > 0.1).
        let mut last = at(start - 0.01, r);
        let mut x = start - 0.01;
        while x < start + span + 1.0 {
            x += 0.01;
            let now = at(x, r);
            assert!((now - last).length() < 1e-2, "{r} at {x}: {last} -> {now}");
            assert!((now.length() - 1.0).abs() < 1e-5);
            last = now;
        }
        assert!((last - r).length() < 1e-6, "infinite past the fade");
    }
}

#[test]
fn a_zero_blend_distance_fades_from_the_face() {
    let mut probe = ReflectionProbe::new(Vec3::ZERO, Vec3::splat(2.0));
    probe.blend_distance = 0.0;
    let r = Vec3::new(-1.0, 0.5, 0.3).normalize();
    let inside = probe.parallax_correct(Vec3::new(1.999, -1.5, 0.5), r);
    let outside = probe.parallax_correct(Vec3::new(2.001, -1.5, 0.5), r);
    assert!((inside - outside).length() < 2e-3, "{inside} / {outside}");
}
