//! The windowed falloff in the bake (#1699): an Area light's pool on a floor
//! fades continuously into its range instead of stepping to zero there.

use glam::Vec3;

use super::area_tests::{direct_at, lit, panel};

/// The baked floor light under a `w × h` panel `y` up with `range`, along +X out
/// to where the floor leaves its range, `n` steps: no step from one to the next
/// may be larger than `tol` of the light under the panel's centre, and the last
/// lit point carries next to nothing.
fn fades_out(w: f32, h: f32, y: f32, range: f32) {
    let scene = lit(vec![panel(w, h, y, range)], vec![]);
    let edge = (range * range - y * y).sqrt();
    let centre = direct_at(&scene, Vec3::ZERO, Vec3::Y);
    let n = 40;
    let at = |k: u32| {
        direct_at(
            &scene,
            Vec3::new(edge * k as f32 / n as f32, 0.0, 0.0),
            Vec3::Y,
        )
    };
    let light: Vec<f32> = (0..=n).map(at).collect();
    for (k, pair) in light.windows(2).enumerate() {
        let step = (pair[0] - pair[1]).abs();
        assert!(
            step <= centre * 0.08,
            "{w}×{h}: step {step} at {k} of {centre}"
        );
    }
    let last_lit = at(n) + direct_at(&scene, Vec3::new(edge * 0.995, 0.0, 0.0), Vec3::Y);
    assert!(
        last_lit <= centre * 1e-3,
        "{w}×{h}: {last_lit} at the edge of {centre}"
    );
    assert_eq!(
        direct_at(&scene, Vec3::new(edge * 1.01, 0.0, 0.0), Vec3::Y),
        0.0
    );
}

#[test]
fn an_area_lights_pool_fades_out_at_its_range_with_no_step() {
    // The review's cases: under the old patch-distance cut they stepped from 16%
    // and 26% of the centre value to zero.
    fades_out(2.0, 1.0, 2.5, 3.0);
    fades_out(4.0, 2.0, 3.0, 4.0);
}
