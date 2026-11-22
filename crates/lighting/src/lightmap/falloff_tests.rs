//! An Area light's pool on a floor fades continuously into its range instead of stepping to zero.

use glam::Vec3;

use super::area_tests::{direct_at, lit, panel};

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
    fades_out(2.0, 1.0, 2.5, 3.0);
    fades_out(4.0, 2.0, 3.0, 4.0);
}

/// A 1400 × (1/d²) hot spot beside a lamp hung 5 cm under the ceiling: one path in 64 hitting it is
/// a speckle unless the path is clamped.
fn bounce_under_a_ceiling_lamp() -> Vec<f32> {
    use super::bake::triangles;
    use super::rays::Rays;
    use super::rng::Rng;
    use super::tests::{floor, quad};
    use super::trace::Tracer;
    use super::{BakeLight, LightShape};
    let p = |x, z| Vec3::new(x, 2.5, z);
    let ceiling = quad(
        7,
        [p(-4.0, -4.0), p(4.0, -4.0), p(4.0, 4.0), p(-4.0, 4.0)],
        Vec3::splat(0.5),
    );
    let lamp = BakeLight {
        shape: LightShape::Point {
            position: Vec3::new(0.0, 2.45, 0.0),
            range: 6.0,
        },
        radiance: Vec3::splat(3.5),
        bakes_direct: true,
        mask_channel: None,
    };
    let scene = lit(vec![lamp], vec![floor(), ceiling]);
    let rays = Rays::build(triangles(&scene));
    let tracer = Tracer::new(&scene, &rays);
    (0..25)
        .map(|k| {
            let at = Vec3::new((k % 5) as f32 * 0.5 - 1.0, 0.0, (k / 5) as f32 * 0.5 - 1.0);
            let mut rng = Rng::new(11, k, 0);
            tracer.texel(at, Vec3::Y, (64, 64), 1, &mut rng).indirect.x
        })
        .collect()
}

#[test]
fn a_lamp_hung_under_a_ceiling_bakes_no_speckle() {
    let light = bounce_under_a_ceiling_lamp();
    let n = light.len() as f32;
    let mean = light.iter().sum::<f32>() / n;
    let spread = (light.iter().map(|e| (e - mean).powi(2)).sum::<f32>() / n).sqrt();
    // One clamped path moves a 64-sample texel by at most 10 / 64.
    let most = super::trace::INDIRECT_CLAMP / 64.0;
    assert!(spread <= most, "spread {spread} over a mean of {mean}");
    assert!(
        light.iter().all(|e| (e - mean).abs() <= 2.0 * most),
        "{light:?}"
    );
}

#[test]
fn the_indirect_clamp_keeps_the_hue() {
    use super::trace::{clamp_indirect, INDIRECT_CLAMP};
    let dim = Vec3::new(1.0, 2.0, 3.0);
    assert_eq!(clamp_indirect(dim), dim);
    let hot = clamp_indirect(Vec3::new(10.0, 20.0, 40.0));
    assert_eq!(hot, Vec3::new(2.5, 5.0, 10.0));
    assert_eq!(hot.max_element(), INDIRECT_CLAMP);
}
