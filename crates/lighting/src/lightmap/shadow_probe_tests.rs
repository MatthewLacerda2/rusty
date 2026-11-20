//! The Area light's shadow probe (#2355): a fully seen or hidden texel casts 20
//! probe rays and 4 or 32 checks, a penumbra the full set plus the corners;
//! `slit_probe_tests` and `probe_check_tests` hold where the probe can be wrong.

use std::sync::atomic::Ordering;

use glam::Vec3;

use super::area_tests::{lit, panel};
use super::bake::triangles;
use super::rays::Rays;
use super::rng::Rng;
use super::tests::quad;
use super::trace::Tracer;
use super::*;

/// A scene and its ray structure, built once and traced by many texels; each
/// call reads the direct shadow rays it alone cast from the count's difference.
pub(super) struct Fixture {
    scene: BakeScene,
    rays: Rays,
}

impl Fixture {
    pub(super) fn new(scene: BakeScene) -> Self {
        let rays = Rays::build(triangles(&scene));
        Self { scene, rays }
    }

    /// The direct irradiance at floor point `x` over `samples` direct samples
    /// from seed `seed`, probed or not, and the direct shadow rays it cast.
    pub(super) fn direct(&self, x: f32, seed: u64, probed: bool, samples: u32) -> (f32, u64) {
        let tracer = Tracer::new(&self.scene, &self.rays);
        let tracer = if probed {
            tracer
        } else {
            tracer.without_probe()
        };
        let before = self.rays.direct.load(Ordering::Relaxed);
        let at = Vec3::new(x, 0.0, 0.1);
        let texel = tracer.texel(at, Vec3::Y, (1, samples), 1, &mut Rng::new(seed, 0, 0));
        let cast = self.rays.direct.load(Ordering::Relaxed) - before;
        (texel.direct.x, cast)
    }
}

/// A 1.2 m panel 3 m up and a blocker over `x < 0` halfway: the floor is fully
/// lit past `x = 0.6`, fully shadowed short of `x = -0.6`, in penumbra between.
fn shadowed() -> Fixture {
    let p = |x, z| Vec3::new(x, 1.5, z);
    let blocker = quad(
        9,
        [p(-4.0, 4.0), p(0.0, 4.0), p(0.0, -4.0), p(-4.0, -4.0)],
        Vec3::ONE,
    );
    Fixture::new(lit(vec![panel(1.2, 0.6, 3.0, 10.0)], vec![blocker]))
}

#[test]
fn fully_lit_and_fully_shadowed_texels_match_the_exhaustive_light_from_a_fraction_of_its_rays() {
    let scene = shadowed();
    let (lit, lit_rays) = scene.direct(1.5, 3, true, 256);
    let (all, all_rays) = scene.direct(1.5, 3, false, 256);
    assert!(
        lit > 0.0 && (lit - all).abs() < all * 0.01,
        "{lit} vs {all}"
    );
    assert!(lit_rays < 36 && all_rays == 256, "{lit_rays} {all_rays}");
    let (dark, dark_rays) = scene.direct(-1.5, 3, true, 256);
    assert_eq!(dark, 0.0, "no check passes, so the dark stays exact");
    assert!(dark_rays < 76, "{dark_rays}");
    assert_eq!(scene.direct(-1.5, 3, false, 256).0, 0.0);
}

/// The mean direct shadow rays a texel at floor point `x` casts over 64 seeds,
/// probed, at `samples` direct samples.
pub(super) fn mean_rays(f: &Fixture, x: f32, samples: u32) -> f32 {
    let rays: u64 = (0..64).map(|s| f.direct(x, s, true, samples).1).sum();
    rays as f32 / 64.0
}

#[test]
fn the_probe_runs_only_where_a_unanimous_texel_casts_fewer_rays_than_the_set() {
    // Rays a texel casts, probed: (direct samples, hidden, lit, penumbra). Up to
    // 49 the set is cast whole (a hidden panel's probe and checks would cost
    // 52); from 64 on, a hidden texel casts 20 + 32 on average, a lit one
    // 20 + 4, a penumbra the set and the four corners.
    let scene = shadowed();
    let table = [
        (17, 25.0, 25.0, 25.0),
        (25, 25.0, 25.0, 25.0),
        (36, 36.0, 36.0, 36.0),
        (49, 49.0, 49.0, 49.0),
        (64, 52.0, 24.0, 68.0),
        (256, 52.0, 24.0, 260.0),
    ];
    for (samples, hidden, lit, penumbra) in table {
        let got = [-1.5, 1.5, 0.0].map(|x| mean_rays(&scene, x, samples));
        println!(
            "{samples}: hidden {}, lit {}, penumbra {}",
            got[0], got[1], got[2]
        );
        for (got, want) in got.into_iter().zip([hidden, lit, penumbra]) {
            assert!(
                (got - want).abs() < 2.5,
                "{samples}: {got} rays, want {want}"
            );
        }
    }
}

#[test]
fn a_penumbra_texel_casts_the_full_set_and_the_corners_and_stays_within_the_exhaustive_noise() {
    let scene = shadowed();
    let seeds = 0..32u64;
    let mean = |probed| {
        let all: Vec<(f32, u64)> = seeds
            .clone()
            .map(|s| scene.direct(0.0, s, probed, 256))
            .collect();
        let e = all.iter().map(|(e, _)| e).sum::<f32>() / all.len() as f32;
        (e, all.iter().map(|(_, r)| r).max().copied())
    };
    let ((probed, rays), (exhaustive, _)) = (mean(true), mean(false));
    let open = Fixture::new(lit(vec![panel(1.2, 0.6, 3.0, 10.0)], vec![]));
    let open = open.direct(0.0, 3, false, 256).0;
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
    for samples in [64, 100, 256] {
        for seed in 0..8 {
            let (_, probed) = scene.direct(0.0, seed, true, samples);
            let (_, all) = scene.direct(0.0, seed, false, samples);
            assert!(probed <= all + 4, "{samples}: {probed} rays against {all}");
        }
    }
}
