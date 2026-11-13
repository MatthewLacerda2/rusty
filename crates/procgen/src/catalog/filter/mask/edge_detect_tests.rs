//! `edge_detect` on a square: a line along its edge only, as wide in tiles at
//! any resolution, rounded on request, and wrapping.

use super::*;
use crate::catalog::filter::test_probe::{assert_wraps, mean, run};
use crate::recipe::OpKind;

fn edges(width: f32, roundness: f32) -> OpKind {
    OpKind::EdgeDetect(EdgeDetect { width, roundness })
}

/// A white square over the middle half of an `n²` tile.
fn square(n: u32) -> Image {
    Image::fill_uv(n, |u, v| {
        let inside = (0.25..0.75).contains(&u) && (0.25..0.75).contains(&v);
        [f32::from(u8::from(inside)), 0.0, 0.0, 1.0]
    })
}

#[test]
fn the_line_sits_on_the_edge_only() {
    let out = run(&edges(0.0625, 0.0), &square(64));
    let row = out.row(32);
    assert_eq!(row[16][0], 1.0, "on the edge, inside");
    assert_eq!(row[15][0], 1.0, "on the edge, outside");
    assert_eq!(row[32][0], 0.0, "the square's middle");
    assert_eq!(row[0][0], 0.0, "far outside");
}

#[test]
fn the_line_covers_the_same_share_of_the_tile_at_any_resolution() {
    // A whole number of pixels each side at both, so neither rounds the line.
    let share = |n| mean(&run(&edges(0.0625, 0.0), &square(n)), 0);
    let (lo, hi) = (share(128), share(256));
    assert!((lo - hi).abs() < 0.02, "{lo} vs {hi}");
}

#[test]
fn roundness_falls_off_from_the_edge() {
    let out = run(&edges(0.25, 1.0), &square(64));
    let row = out.row(32);
    assert!(
        row[16][0] > row[19][0] && row[19][0] > row[22][0],
        "{:?}",
        &row[14..24]
    );
    assert!(row[22][0] > 0.0);
}

#[test]
fn a_mask_with_no_edge_reads_black() {
    for fill in [0.0, 1.0] {
        let out = run(&edges(0.1, 0.5), &Image::filled(16, [fill, 0.0, 0.0, 1.0]));
        assert!(out.pixels().iter().all(|p| p[0] == 0.0), "fill {fill}");
    }
}

#[test]
fn it_wraps() {
    assert_wraps(&edges(0.1, 0.5), &square(32));
}

#[test]
fn a_line_thinner_than_a_pixel_fades_rather_than_vanishing() {
    // 10 px at 2048², under a pixel at 128²: it reads, at about its share.
    let out = run(&edges(0.005, 0.0), &square(128));
    let line = mean(&out, 0);
    assert!(line > 0.0, "a sub-pixel line baked black");
    // Four sides of half the tile, 0.005 wide: 0.01 of the tile.
    assert!((line - 0.01).abs() < 0.002, "{line}");
}

#[test]
fn a_thin_line_keeps_its_mean_across_resolutions() {
    for roundness in [0.0, 1.0] {
        let share = |n| mean(&run(&edges(0.005, roundness), &square(n)), 0);
        let (lo, mid, hi) = (share(128), share(256), share(512));
        let near = |a: f32, b: f32| (a - b).abs() < 0.15 * b;
        assert!(
            near(lo, hi) && near(mid, hi),
            "{roundness}: {lo} {mid} {hi}"
        );
    }
}

#[test]
fn a_width_out_of_range_neither_panics_nor_reads_nan() {
    for width in [f32::NAN, -1.0, f32::INFINITY] {
        let out = run(&edges(width, 0.5), &square(16));
        assert!(out.pixels().iter().all(|p| p[0].is_finite()), "{width}");
    }
}
