//! The Area light's shadow probe (#2355): a texel fully seeing a panel gets the
//! exhaustive estimate's light from 20 shadow rays (four corners and one point
//! in each of 4 × 4 blocks), one fully hidden from it from 52 (two more rounds
//! confirm the dark), and one in its penumbra the full set plus the corners, so
//! the probe changes the cost, not the light; `area_tests`' penumbra widths hold
//! it at a shadow's edges, `slit_probe_tests` through a gap.

use std::sync::atomic::Ordering;

use glam::Vec3;

use super::area_tests::{lit, panel};
use super::bake::triangles;
use super::rays::Rays;
use super::rng::Rng;
use super::tests::quad;
use super::trace::Tracer;
use super::*;

/// A 1.2 m panel 3 m up and a blocker over `x < 0` halfway: the floor is fully
/// lit past `x = 0.6`, fully shadowed short of `x = -0.6`, in penumbra between.
fn shadowed() -> BakeScene {
    let p = |x, z| Vec3::new(x, 1.5, z);
    let blocker = quad(
        9,
        [p(-4.0, 4.0), p(0.0, 4.0), p(0.0, -4.0), p(-4.0, -4.0)],
        Vec3::ONE,
    );
    lit(vec![panel(1.2, 0.6, 3.0, 10.0)], vec![blocker])
}

/// The direct irradiance at floor point `x` over 256 direct samples from seed
/// `seed`, probed or not, and the direct shadow rays it cast.
fn direct(scene: &BakeScene, x: f32, seed: u64, probed: bool) -> (f32, u64) {
    direct_over(scene, x, seed, probed, 256)
}

/// [`direct`] over `samples` direct samples.
fn direct_over(scene: &BakeScene, x: f32, seed: u64, probed: bool, samples: u32) -> (f32, u64) {
    let rays = Rays::build(triangles(scene));
    let tracer = Tracer::new(scene, &rays);
    let tracer = if probed {
        tracer
    } else {
        tracer.without_probe()
    };
    let mut rng = Rng::new(seed, 0, 0);
    let at = Vec3::new(x, 0.0, 0.1);
    let e = tracer
        .texel(at, Vec3::Y, (1, samples), 1, &mut rng)
        .direct
        .x;
    (e, rays.direct.load(Ordering::Relaxed))
}

#[test]
fn fully_lit_and_fully_shadowed_texels_match_the_exhaustive_light_from_a_fraction_of_its_rays() {
    let scene = shadowed();
    let (lit, lit_rays) = direct(&scene, 1.5, 3, true);
    let (all, all_rays) = direct(&scene, 1.5, 3, false);
    assert!(
        lit > 0.0 && (lit - all).abs() < all * 0.01,
        "{lit} vs {all}"
    );
    assert_eq!((lit_rays, all_rays), (4 + 16, 256));
    let (dark, dark_rays) = direct(&scene, -1.5, 3, true);
    assert_eq!(
        (dark, dark_rays),
        (0.0, 4 + 3 * 16),
        "three rounds confirm the dark"
    );
    assert_eq!(direct(&scene, -1.5, 3, false).0, 0.0);
}

#[test]
fn a_penumbra_texel_casts_the_full_set_and_the_corners_and_stays_within_the_exhaustive_noise() {
    let scene = shadowed();
    let seeds = 0..32u64;
    let mean = |probed| {
        let all: Vec<(f32, u64)> = seeds
            .clone()
            .map(|s| direct(&scene, 0.0, s, probed))
            .collect();
        let e = all.iter().map(|(e, _)| e).sum::<f32>() / all.len() as f32;
        (e, all.iter().map(|(_, r)| r).max().copied())
    };
    let ((probed, rays), (exhaustive, _)) = (mean(true), mean(false));
    let open = direct(
        &lit(vec![panel(1.2, 0.6, 3.0, 10.0)], vec![]),
        0.0,
        3,
        false,
    )
    .0;
    let error = (probed - exhaustive).abs() / open;
    println!("4×4 probe in the penumbra: {probed} vs exhaustive {exhaustive}, {error:.4} of open");
    assert!(error < 0.01, "{probed} vs {exhaustive} ({open} open)");
    assert_eq!(rays, Some(4 + 256), "the corners, then every point once");
}

#[test]
fn where_the_full_set_follows_the_probe_costs_only_its_corners() {
    // The probe's block points are points of the set, counted toward it, so a
    // penumbra texel casts at most the four corners more than the exhaustive
    // estimate, from the smallest count the probe runs at up.
    let scene = shadowed();
    for samples in [25, 32, 64] {
        for seed in 0..8 {
            let (_, probed) = direct_over(&scene, 0.0, seed, true, samples);
            let (_, all) = direct_over(&scene, 0.0, seed, false, samples);
            assert!(probed <= all + 4, "{samples}: {probed} rays against {all}");
        }
    }
}
