use glam::Vec3;

use super::super::bake::triangles;
use super::super::input::{BakeMesh, BakeScene};
use super::super::rays::Rays;
use super::super::rng::Rng;
use super::super::tests::quad;
use super::Tracer;

fn face(n: Vec3, glow: f32, albedo: f32) -> BakeMesh {
    let (t, b) = (n.any_orthonormal_pair().0, n.any_orthonormal_pair().1);
    let c = -n;
    let mut m = quad(
        0,
        [c - t - b, c + t - b, c + t + b, c - t + b],
        Vec3::splat(albedo),
    );
    m.runs[0].emissive = Vec3::splat(glow);
    m
}

fn dark(meshes: Vec<BakeMesh>) -> BakeScene {
    BakeScene {
        meshes,
        sky: Vec3::ZERO.into(),
        ..Default::default()
    }
}

fn indirect(scene: &BakeScene, p: Vec3, n: Vec3, samples: u32, bounces: u32, seed: u64) -> f32 {
    let rays = Rays::build(triangles(scene));
    let tracer = Tracer::new(scene, &rays);
    let mut rng = Rng::new(seed, 0, 0);
    tracer
        .texel(p, n, (samples, samples), bounces, &mut rng)
        .indirect
        .x
}

#[test]
fn a_glowing_box_bakes_the_furnace_sum_at_any_depth() {
    let (glow, albedo) = (0.5, 0.8);
    let axes = [
        Vec3::X,
        Vec3::NEG_X,
        Vec3::Y,
        Vec3::NEG_Y,
        Vec3::Z,
        Vec3::NEG_Z,
    ];
    let scene = dark(axes.map(|n| face(n, glow, albedo)).to_vec());
    let at = Vec3::new(0.3, -1.0, -0.2);
    for bounces in [1, 2, 8] {
        let expected: f32 = (0..bounces).map(|k| glow * albedo.powi(k as i32)).sum();
        let got = (0..8)
            .map(|seed| indirect(&scene, at, Vec3::Y, 4096, bounces, seed))
            .sum::<f32>()
            / 8.0;
        let err = (got - expected).abs() / expected;
        assert!(err < 0.03, "{bounces} bounces: {got} vs {expected}");
    }
}

fn panel(side: f32, glow: f32) -> BakeMesh {
    let h = side / 2.0;
    let p = |x, z| Vec3::new(x, 1.0, z);
    let mut m = quad(2, [p(-h, -h), p(h, -h), p(h, h), p(-h, h)], Vec3::ONE);
    m.runs[0].emissive = Vec3::splat(glow);
    m
}

#[test]
fn a_small_bright_panel_bakes_smooth_and_true() {
    let (side, glow) = (0.3, 5.0);
    let scene = dark(vec![panel(side, glow)]);
    let n = 64;
    let cell = side / n as f32;
    let exact: f32 = (0..n * n)
        .map(|k| {
            let at = Vec3::new(((k % n) as f32 + 0.5) * cell - side / 2.0, 1.0, 0.0)
                + Vec3::Z * (((k / n) as f32 + 0.5) * cell - side / 2.0);
            let cos = at.normalize().y;
            glow * cos * cos / at.length_squared() * cell * cell
        })
        .sum::<f32>()
        / std::f32::consts::PI;
    let runs: Vec<f32> = (0..24)
        .map(|seed| indirect(&scene, Vec3::ZERO, Vec3::Y, 64, 1, seed))
        .collect();
    let mean = runs.iter().sum::<f32>() / runs.len() as f32;
    let var = runs.iter().map(|v| (v - mean).powi(2)).sum::<f32>() / runs.len() as f32;
    assert!((mean - exact).abs() < exact * 0.02, "{mean} vs {exact}");
    assert!(var.sqrt() < exact * 0.05, "noise {} of {exact}", var.sqrt());
}

#[test]
fn glowing_panels_and_many_lamps_bake_the_same_bytes_on_any_worker_count() {
    use super::super::bake::bake_on;
    use super::super::input::{BakeLight, LightShape};
    use super::super::tests::{floor, settings};
    let mut scene = dark(vec![floor(), panel(0.3, 5.0), panel(1.0, 0.5)]);
    scene.lights = (0..40)
        .map(|k| BakeLight {
            shape: LightShape::Point {
                position: Vec3::new(k as f32 * 0.1 - 2.0, 1.5, 0.5),
                range: 4.0,
            },
            radiance: Vec3::splat(0.1),
            bakes_direct: k % 2 == 0,
            mask_channel: None,
        })
        .collect();
    let progress = Default::default();
    let on = |threads| bake_on(&scene, &settings(), &progress, threads);
    assert_eq!(on(1), on(4));
}
