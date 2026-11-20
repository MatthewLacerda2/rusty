//! The Area light's shadow probe where it can be wrong (#2355): a panel seen
//! only through a slit that shows none of its corners, and one with a thin rod
//! under it that hides none. A probe whose points all miss the slit calls the
//! panel hidden, one whose points all miss the rod calls it seen; the checks
//! (`trace::check`) find most such verdicts wrong, and the texel then casts the
//! whole set. These hold how often a verdict survives where it is wrong.
//! And a panel in a recess whose walls meet its edges, which the corner rays
//! must not graze.

use glam::Vec3;

use super::area_tests::{lit, panel};
use super::shadow_probe_tests::{direct_over, mean_rays};
use super::tests::quad;
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

/// The share of `seeds` seeds at each floor point of `xs`, at `samples` direct
/// samples, whose probe and checks all missed what `scene` puts across the
/// panel: those that cast fewer rays than the set, a unanimous verdict kept.
fn miss_rate(scene: &BakeScene, xs: &[f32], seeds: u64, samples: u32) -> f64 {
    let missed = xs
        .iter()
        .flat_map(|&x| (0..seeds).map(move |s| (x, s)))
        .filter(|&(x, s)| direct_over(scene, x, s, true, samples).1 < samples as u64)
        .count();
    missed as f64 / (xs.len() as u64 * seeds) as f64
}

#[test]
fn seen_through_a_slit_a_texel_rarely_keeps_the_probes_dark_verdict() {
    // Each of these texels sees the panel only through the slit. Measured over
    // 31 points and 1000 seeds: 1.8% at 256 samples, against 2.9% for one point
    // a block in two confirming rounds; checks drawn at random left a third.
    let rate = miss_rate(&slit(0.0625), &[-0.1, 0.0, 0.2], 256, 256);
    assert!(rate < 0.03, "slit 1/16: {rate} of texels black");
}

#[test]
fn under_a_thin_rod_a_texel_rarely_loses_its_shadow() {
    // Measured over 11 points and 1000 seeds at 256 samples: rod 1/8 5.2% and
    // rod 1/16 26% (10% and 36% with two confirming rounds and no checks).
    for (frac, bound) in [(0.125, 0.07), (0.0625, 0.31)] {
        let rate = miss_rate(&under(frac * 0.6, 1.0), &[-0.2, 0.0, 0.2], 256, 256);
        assert!(rate < bound, "rod {frac}: {rate} of texels unshadowed");
    }
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
