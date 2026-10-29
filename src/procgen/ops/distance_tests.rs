//! `distance` (#1492): exact against a brute-force periodic search, analytic on a
//! circle, and tiling across the wrap.

use super::*;
use crate::procgen::hash;

const RES: u32 = 64;

/// A mask of the disc of `radius` tiles around `(cu, cv)`, measured across the wrap.
fn disc(cu: f32, cv: f32, radius: f32) -> Image {
    Image::fill_uv(RES, |u, v| {
        let wrap = |d: f32| (d + 0.5).rem_euclid(1.0) - 0.5;
        let inside = wrap(u - cu).hypot(wrap(v - cv)) < radius;
        let m = if inside { 1.0 } else { 0.0 };
        [m, m, m, 1.0]
    })
}

#[test]
fn a_disc_reads_its_analytic_distance_to_the_rim_within_a_pixel() {
    let out = distance(&disc(0.5, 0.5, 0.25), Field::Const(0.25));
    let pixel = 1.0 / RES as f32 / 0.25;
    for (i, p) in out.pixels().iter().enumerate() {
        let (u, v) = ((i as u32 % RES) as f32 + 0.5, (i as u32 / RES) as f32 + 0.5);
        let r = (u / RES as f32 - 0.5).hypot(v / RES as f32 - 0.5);
        let analytic = ((0.25 - r) / 0.25).max(0.0);
        assert!(
            (p[0] - analytic).abs() <= pixel,
            "pixel {i}: {} vs {analytic}",
            p[0]
        );
    }
}

#[test]
fn a_mask_across_the_wrap_reads_as_the_same_mask_centred() {
    // The disc around the tile's corner is the centred disc moved half a tile, so
    // its distances must be the centred ones moved too, exactly.
    let (centred, corner) = (disc(0.5, 0.5, 0.3), disc(0.0, 0.0, 0.3));
    let (a, b) = (
        distance(&centred, Field::Const(0.2)),
        distance(&corner, Field::Const(0.2)),
    );
    let half = RES as i64 / 2;
    for y in 0..RES as i64 {
        for x in 0..RES as i64 {
            assert_eq!(b.get_wrapped(x, y), a.get_wrapped(x + half, y + half));
        }
    }
}

#[test]
fn a_noisy_mask_matches_a_brute_force_periodic_search() {
    let n = 24i64;
    let mask = Image::fill_uv(n as u32, |u, v| {
        let (x, y) = ((u * n as f32) as i64, (v * n as f32) as i64);
        let m = if hash::unit2(x, y, 0, 7) < 0.8 {
            1.0
        } else {
            0.0
        };
        [m, 0.0, 0.0, 1.0]
    });
    let out = distance(&mask, Field::Const(1.0));
    let wrapped = |d: i64| d.rem_euclid(n).min((-d).rem_euclid(n)) as f32;
    for y in 0..n {
        for x in 0..n {
            let nearest = (0..n * n)
                .filter(|&i| mask.pixels()[i as usize][0] < 0.5)
                .map(|i| wrapped(i % n - x).hypot(wrapped(i / n - y)))
                .fold(f32::INFINITY, f32::min);
            let want = if nearest == 0.0 {
                0.0
            } else {
                (nearest - 0.5) / n as f32
            };
            let got = out.get_wrapped(x, y)[0];
            assert!((got - want).abs() < 1e-6, "({x}, {y}): {got} vs {want}");
        }
    }
}

#[test]
fn an_all_inside_mask_reads_one_and_an_empty_mask_zero() {
    assert_eq!(
        distance(&Image::filled(8, [1.0; 4]), Field::Const(0.1)),
        Image::filled(8, [1.0; 4])
    );
    let empty = Image::filled(8, [0.0, 0.0, 0.0, 1.0]);
    assert_eq!(distance(&empty, Field::Const(0.1)), empty);
}
