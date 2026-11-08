//! Emissive surfaces as lights (#1883): a closed glowing box is the furnace test
//! (its light after `B` bounces is known exactly, so emitter sampling and Russian
//! roulette are both held unbiased), and a small bright panel bakes smooth at a
//! sample count a bounce ray's luck could not.

use glam::Vec3;

use super::super::bake::triangles;
use super::super::input::{BakeMesh, BakeScene};
use super::super::rays::Rays;
use super::super::rng::Rng;
use super::super::tests::quad;
use super::Tracer;

/// The inside face of the box `[-1, 1]³` facing `n` (an axis), glowing `glow`.
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

/// The bounce part a texel at `p` facing `n` bakes in `scene`, over `samples`.
fn indirect(scene: &BakeScene, p: Vec3, n: Vec3, samples: u32, bounces: u32, seed: u64) -> f32 {
    let rays = Rays::build(triangles(scene));
    let tracer = Tracer::new(scene, &rays);
    let mut rng = Rng::new(seed, 0, 0);
    tracer.texel(p, n, samples, bounces, &mut rng).indirect.x
}

#[test]
fn a_glowing_box_bakes_the_furnace_sum_at_any_depth() {
    // Dim, so no path reaches `INDIRECT_CLAMP`: the clamp cuts the tail of a
    // vertex in a corner (by design), which would read here as bias.
    let (glow, albedo) = (0.02, 0.8);
    let axes = [
        Vec3::X,
        Vec3::NEG_X,
        Vec3::Y,
        Vec3::NEG_Y,
        Vec3::Z,
        Vec3::NEG_Z,
    ];
    let scene = dark(axes.map(|n| face(n, glow, albedo)).to_vec());
    // Seen from inside, every direction meets glow `L`: the texel's own view is
    // `L`, and each bounce adds `albedo` times the last, `bounces` deep. Past the
    // second, Russian roulette ends paths early; unbiased, it keeps the sum.
    let at = Vec3::new(0.3, -1.0, -0.2);
    for bounces in [1, 2, 8] {
        let expected: f32 = (0..=bounces).map(|k| glow * albedo.powi(k as i32)).sum();
        let got = (0..8)
            .map(|seed| indirect(&scene, at, Vec3::Y, 4096, bounces, seed))
            .sum::<f32>()
            / 8.0;
        let err = (got - expected).abs() / expected;
        assert!(err < 0.03, "{bounces} bounces: {got} vs {expected}");
    }
}

/// A `side`² panel glowing `glow`, one unit over the origin, facing down.
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
    // Its light at the origin, `L ∫ cos cos / d² dA / π`, on a fine grid.
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
    // A bounce ray finds the panel one time in ~35: at 64 samples that alone is
    // ~73% noise. Aimed at, it is a few percent.
    assert!(var.sqrt() < exact * 0.05, "noise {} of {exact}", var.sqrt());
}
