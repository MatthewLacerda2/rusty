//! The Area light's shadow probe seen through a gap (#2355): a panel above a
//! cover with one slit, so the floor sees the panel only through a strip that
//! shows none of its corners. A probe whose points all miss the strip would
//! call the panel hidden and bake the texel black, a bias no sample count
//! removes; the confirming rounds (`trace::own`, `ROUNDS`) keep the probed
//! floor inside the exhaustive estimate's own noise.

use std::sync::atomic::Ordering;

use glam::Vec3;

use super::area_tests::{lit, panel};
use super::bake::triangles;
use super::rays::Rays;
use super::rng::Rng;
use super::tests::quad;
use super::trace::Tracer;
use super::*;

/// The 1.2 × 0.6 m panel 3 m up, under a cover 0.3 m below it whose one slit,
/// along the panel's short side, shows `frac` of the panel's width straight
/// down (the cover is nearer, so the slit is `0.9 ×` that wide).
fn slit(frac: f32) -> BakeScene {
    let half = frac * 1.2 * 0.9 / 2.0;
    let p = |x, z| Vec3::new(x, 2.7, z);
    let cover = |a: f32, b: f32, entity| {
        quad(
            entity,
            [p(a, 4.0), p(b, 4.0), p(b, -4.0), p(a, -4.0)],
            Vec3::ONE,
        )
    };
    let meshes = vec![cover(-4.0, -half, 9), cover(half, 4.0, 10)];
    lit(vec![panel(1.2, 0.6, 3.0, 10.0)], meshes)
}

/// The floor's direct light at 31 points from `x = -1.5` to `1.5` over 256
/// direct samples, each the mean of 16 seeds from `seed`, probed or not, and
/// the direct shadow rays the 31 × 16 texels cast.
fn floor(scene: &BakeScene, seed: u64, probed: bool) -> (Vec<f32>, u64) {
    let rays = Rays::build(triangles(scene));
    let tracer = Tracer::new(scene, &rays);
    let tracer = if probed {
        tracer
    } else {
        tracer.without_probe()
    };
    let light = (0..31)
        .map(|i| {
            let at = Vec3::new(-1.5 + i as f32 * 0.1, 0.0, 0.1);
            let seeds = seed..seed + 16;
            let sum: f32 = seeds
                .map(|s| {
                    let mut rng = Rng::new(s, 0, 0);
                    tracer.texel(at, Vec3::Y, (1, 256), 1, &mut rng).direct.x
                })
                .sum();
            sum / 16.0
        })
        .collect();
    (light, rays.direct.load(Ordering::Relaxed))
}

/// The mean and worst relative difference of `of` from `truth` over the texels
/// `truth` lights.
fn error(of: &[f32], truth: &[f32]) -> (f32, f32) {
    let lit: Vec<f32> = of
        .iter()
        .zip(truth)
        .filter(|(_, t)| **t > 0.0)
        .map(|(e, t)| (e - t).abs() / t)
        .collect();
    let mean = lit.iter().sum::<f32>() / lit.len() as f32;
    (mean, lit.iter().copied().fold(0.0, f32::max))
}

#[test]
fn seen_through_a_slit_the_probed_floor_stays_within_the_exhaustive_noise() {
    for frac in [0.25, 0.125, 0.0625] {
        let scene = slit(frac);
        let (truth, exhaustive_rays) = floor(&scene, 0, false);
        let (again, _) = floor(&scene, 100, false);
        let (probed, probed_rays) = floor(&scene, 100, true);
        assert!(
            truth.iter().all(|e| *e > 0.0),
            "{frac}: the slit lights every point"
        );
        let (noise, worst_noise) = error(&again, &truth);
        let (bias, worst) = error(&probed, &truth);
        println!(
            "slit {frac}: probed off by {bias:.4} (worst {worst:.4}), a second exhaustive \
             run by {noise:.4} (worst {worst_noise:.4}); {} rays a texel against {}",
            probed_rays as f32 / 496.0,
            exhaustive_rays as f32 / 496.0,
        );
        assert!(
            bias < noise * 1.5 + 0.005,
            "{frac}: off by {bias} against the exhaustive noise {noise}"
        );
    }
}
