use super::*;

fn op(scale: f32, anisotropy: f32, angle: f32) -> AnisotropicNoise {
    AnisotropicNoise {
        scale,
        anisotropy,
        angle,
        seed: 0,
    }
}

/// Mean absolute change of the sampler over a small step along `(du, dv)`.
fn roughness(f: &Sampler, (du, dv): (f32, f32)) -> f32 {
    let n = 64;
    let mut acc = 0.0;
    for i in 0..n {
        for j in 0..n {
            let (u, v) = (i as f32 / n as f32, j as f32 / n as f32);
            acc += (f(u + du, v + dv)[0] - f(u, v)[0]).abs();
        }
    }
    acc / (n * n) as f32
}

#[test]
fn fibres_run_along_the_angle() {
    for (angle, along, across) in [
        (0.0, (0.01, 0.0), (0.0, 0.01)),
        (0.25, (0.0, 0.01), (0.01, 0.0)),
        (0.125, (0.007, 0.007), (0.007, -0.007)),
    ] {
        let f = sampler(&op(32.0, 8.0, angle), 3);
        let (l, c) = (roughness(&f, along), roughness(&f, across));
        assert!(c > 3.0 * l, "angle {angle}: along {l}, across {c}");
    }
}

#[test]
fn it_repeats_every_whole_tile_at_any_angle() {
    for angle in [0.0, 0.03, 0.1, 0.2, 0.37, 0.61, 0.9] {
        let f = sampler(&op(13.0, 6.0, angle), 9);
        for i in 0..50 {
            let (u, v) = (hash::unit2(i, 0, 2, 1), hash::unit2(i, 1, 2, 1));
            for (du, dv) in [(1.0, 0.0), (0.0, 1.0), (-1.0, 1.0)] {
                let (a, b) = (f(u, v)[0], f(u + du, v + dv)[0]);
                assert!(
                    (a - b).abs() < 2e-3,
                    "angle {angle} at ({u},{v}): {a} vs {b}"
                );
            }
        }
    }
}

#[test]
fn the_snap_lands_within_its_documented_error() {
    for i in 0..360 {
        let angle = i as f32 / 720.0;
        let (a, b) = snap(angle, 11.0);
        let got = (b as f32).atan2(a as f32);
        let off = (got - (angle * TAU).rem_euclid(PI)).abs();
        let off = off.min(PI - off).to_degrees();
        assert!(
            off <= 3.6,
            "angle {angle}: snapped to ({a},{b}), {off}° off"
        );
    }
    assert_eq!(snap(0.1, 1.0), (1, 0), "a low scale keeps only the axes");
}

#[test]
fn malformed_params_are_bounded() {
    for x in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, 1e30, -1e30, 0.0] {
        let f = sampler(&op(x, x, x), 1);
        let y = f(0.3, 0.6)[0];
        assert!((0.0..=1.0).contains(&y), "{x}: {y}");
    }
}
