//! How often the Area light's shadow probe's wrong verdicts survive the checks (`trace::check`): a
//! slit showing none of the panel's corners, a thin rod hiding none; and a recess whose walls meet
//! the panel's edges.

use glam::Vec3;

use super::area_tests::{lit, panel};
use super::shadow_probe_tests::{mean_rays, Fixture};
use super::tests::quad;
use super::*;

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

pub(super) fn under(hx: f32, hz: f32) -> BakeScene {
    let p = |x, z| Vec3::new(x, 2.99, z);
    let q = quad(
        9,
        [p(-hx, hz), p(hx, hz), p(hx, -hz), p(-hx, -hz)],
        Vec3::ONE,
    );
    lit(vec![panel(1.2, 0.6, 3.0, 10.0)], vec![q])
}

fn miss_rate(scene: &Fixture, xs: &[f32], seeds: u64, samples: u32) -> f64 {
    let missed = xs
        .iter()
        .flat_map(|&x| (0..seeds).map(move |s| (x, s)))
        .filter(|&(x, s)| scene.direct(x, s, true, samples).1 < samples as u64)
        .count();
    missed as f64 / (xs.len() as u64 * seeds) as f64
}

#[test]
fn seen_through_a_slit_a_texel_rarely_keeps_the_probes_dark_verdict() {
    // Measured over 31 points and 1000 seeds: 1.8% at 256 samples; checks drawn at random left a
    // third.
    let rate = miss_rate(&Fixture::new(slit(0.0625)), &[-0.1, 0.0, 0.2], 256, 256);
    assert!(rate < 0.03, "slit 1/16: {rate} of texels black");
}

#[test]
fn under_a_thin_rod_a_texel_rarely_loses_its_shadow() {
    // Measured over 11 points and 1000 seeds at 256 samples: rod 1/8 5.2%, rod 1/16 26%.
    for (frac, bound) in [(0.125, 0.07), (0.0625, 0.31)] {
        let rate = miss_rate(
            &Fixture::new(under(frac * 0.6, 1.0)),
            &[-0.2, 0.0, 0.2],
            256,
            256,
        );
        assert!(rate < bound, "rod {frac}: {rate} of texels unshadowed");
    }
}

#[test]
fn a_panel_flush_in_a_recess_seen_whole_takes_the_probes_cheap_path() {
    // A corner ray aimed at the panel's very edge grazes the walls, so the probe aims its corners
    // inside.
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
    let scene = Fixture::new(lit(vec![panel(1.2, 0.6, 3.0, 10.0)], walls));
    for x in [0.0, 0.05] {
        let rays = mean_rays(&scene, x, 256);
        assert!(
            rays < 40.0,
            "a texel seeing all the panel at {x}: {rays} rays"
        );
    }
}
