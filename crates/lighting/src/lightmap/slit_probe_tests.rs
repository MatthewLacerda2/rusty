//! The Area light's shadow probe where it can be wrong (#2355): a panel seen
//! only through a slit that shows none of its corners, and one with a thin rod
//! under it that hides none. A probe whose points all miss the slit calls the
//! panel hidden, one whose points all miss the rod calls it seen; the checks
//! (`trace::own`, `LIT_CHECKS`, `HIDDEN_CHECKS`) correct either verdict on
//! average, so the probed light's mean is the exhaustive one's, with more noise.
//! And a panel in a recess whose walls meet its edges, which the corner rays
//! must not graze.

use glam::Vec3;

use super::area_tests::{lit, panel};
use super::bake::triangles;
use super::rays::Rays;
use super::rng::Rng;
use super::shadow_probe_tests::mean_rays;
use super::tests::quad;
use super::trace::Tracer;
use super::*;

/// The 1.2 × 0.6 m panel 3 m up, under a cover 0.3 m below it whose one slit,
/// along the panel's short side, shows `frac` of the panel's width straight
/// down (the cover is nearer, so the slit is `0.9 ×` that wide).
pub(super) fn slit(frac: f32) -> BakeScene {
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

/// The same panel with a `2 hx × 2 hz` m occluder 1 cm under its centre: a rod
/// across it when `hz` passes its edges, a blob when not.
pub(super) fn under(hx: f32, hz: f32) -> BakeScene {
    let p = |x, z| Vec3::new(x, 2.99, z);
    let q = quad(
        9,
        [p(-hx, hz), p(hx, hz), p(hx, -hz), p(-hx, -hz)],
        Vec3::ONE,
    );
    lit(vec![panel(1.2, 0.6, 3.0, 10.0)], vec![q])
}

/// The mean and the standard error of the direct light at floor point `x` over
/// `seeds` seeds at 256 direct samples, probed or not.
fn light(scene: &BakeScene, x: f32, seeds: u64, probed: bool) -> (f64, f64) {
    let rays = Rays::build(triangles(scene));
    let tracer = Tracer::new(scene, &rays);
    let tracer = if probed {
        tracer
    } else {
        tracer.without_probe()
    };
    let at = Vec3::new(x, 0.0, 0.1);
    let all: Vec<f64> = (0..seeds)
        .map(|s| {
            let mut rng = Rng::new(s + 1000 * probed as u64, 0, 0);
            tracer.texel(at, Vec3::Y, (1, 256), 1, &mut rng).direct.x as f64
        })
        .collect();
    let n = all.len() as f64;
    let mean = all.iter().sum::<f64>() / n;
    let var = all.iter().map(|e| (e - mean).powi(2)).sum::<f64>() / (n - 1.0);
    (mean, (var / n).sqrt())
}

/// Asserts the probed mean at `x` is the exhaustive one within three standard
/// errors of their difference and within `within` of it, relatively.
fn holds(scene: &BakeScene, x: f32, seeds: u64, within: f64, what: &str) {
    let (truth, truth_se) = light(scene, x, seeds, false);
    let (probed, probed_se) = light(scene, x, seeds, true);
    let se = truth_se.hypot(probed_se);
    println!(
        "{what} at {x}: probed {probed:.5} ± {probed_se:.5}, exhaustive {truth:.5} ± {truth_se:.5}"
    );
    assert!(truth > 0.0, "{what}: the floor at {x} is lit");
    assert!(
        (probed - truth).abs() < 3.0 * se && (probed - truth).abs() < within * truth,
        "{what} at {x}: probed {probed} against {truth} (± {se})"
    );
}

#[test]
fn seen_through_a_slit_the_probed_floors_mean_light_is_the_exhaustive_ones() {
    // Without the checks, the probe called a third of these samples black.
    for x in [-0.1, 0.0, 0.2] {
        holds(&slit(0.0625), x, 128, 0.15, "slit 1/16");
    }
}

#[test]
fn under_a_thin_rod_the_probed_floor_keeps_its_shadow_on_average() {
    // The rod hides 1/16 of the panel's width: about 6% of its light straight
    // under it. All 20 probe rays miss it on about a third of texels, which,
    // left unchecked, gave back about 2% of the light; the checks hold the
    // mean to the exhaustive one well inside that.
    let rod = under(0.0625 * 0.6, 1.0);
    holds(&rod, 0.0, 256, 0.012, "rod 1/16");
}

#[test]
fn a_panel_flush_in_a_recess_seen_whole_takes_the_probes_cheap_path() {
    // The recess's walls meet the panel's plane at its very edge: a corner ray
    // aimed there grazes them, so the probe aims its corners inside the panel.
    let (hx, hz, top, bottom) = (0.6, 0.3, 3.0, 2.8);
    let wall = |a: (f32, f32), b: (f32, f32), entity| {
        let c = |(x, z): (f32, f32), y| Vec3::new(x, y, z);
        quad(
            entity,
            [c(a, top), c(b, top), c(b, bottom), c(a, bottom)],
            Vec3::ONE,
        )
    };
    let corners = [(-hx, -hz), (hx, -hz), (hx, hz), (-hx, hz)];
    let walls = (0..4)
        .map(|i| wall(corners[i], corners[(i + 1) % 4], 9 + i as u32))
        .collect();
    let scene = lit(vec![panel(1.2, 0.6, 3.0, 10.0)], walls);
    for x in [0.0, 0.05] {
        let rays = mean_rays(&scene, x, 256);
        assert!(
            rays < 40.0,
            "a texel seeing all the panel at {x}: {rays} rays"
        );
    }
}
