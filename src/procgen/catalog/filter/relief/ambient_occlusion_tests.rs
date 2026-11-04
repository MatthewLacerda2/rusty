//! `ambient_occlusion` on primitives: flat ground and peaks see the sky, the foot
//! of a wall and a wide groove's floor do not, and it wraps, holds across
//! resolutions and bakes the same twice.

use super::*;
use crate::procgen::catalog::filter::test_probe::{assert_wraps, relief, run};
use crate::procgen::recipe::OpKind;

fn ao(radius: f32) -> OpKind {
    OpKind::AmbientOcclusion(AmbientOcclusion {
        radius,
        depth: 0.1,
        quality: 8,
    })
}

/// A raised band across the middle third of an `n²` tile: a groove 2/3 wide
/// between its walls, wrapped.
fn band(n: u32) -> Image {
    Image::fill_uv(n, |u, _| {
        [f32::from(u8::from((1.0 / 3.0..2.0 / 3.0).contains(&u))); 4]
    })
}

#[test]
fn flat_ground_sees_the_whole_sky() {
    let out = run(&ao(0.1), &Image::filled(16, [0.7; 4]));
    assert!(out.pixels().iter().all(|p| p[0] == 1.0));
}

#[test]
fn the_foot_of_a_wall_is_darker_than_open_floor_and_the_top() {
    let n = 96;
    let out = run(&ao(0.1), &band(n));
    let row = out.row(0);
    let (foot, floor, top) = (row[31][0], row[80][0], row[48][0]);
    assert!(foot < floor - 0.1, "foot {foot} vs open floor {floor}");
    assert_eq!(top, 1.0, "the band's top sees the sky");
    assert_eq!(floor, 1.0, "a wall beyond the radius hides nothing");
}

#[test]
fn a_wide_groove_is_occluded_where_cavity_would_not_reach() {
    // At radius 0.3 the walls hide some sky from the middle of the floor.
    let out = run(&ao(0.3), &band(96));
    assert!(out.row(0)[80][0] < 0.99);
}

#[test]
fn it_reads_alike_at_any_resolution() {
    let (lo, hi) = (run(&ao(0.1), &band(96)), run(&ao(0.1), &band(192)));
    // Pixel x at 96² covers pixels 2x and 2x + 1 at 192².
    for x in 0..96 {
        let a = lo.row(0)[x][0];
        let b = (hi.row(0)[2 * x][0] + hi.row(0)[2 * x + 1][0]) / 2.0;
        assert!((a - b).abs() < 0.05, "pixel {x}: {a} vs {b}");
    }
}

#[test]
fn heights_sample_as_the_image_does() {
    let img = relief(8);
    let heights = Heights::of(&img);
    for (u, v) in [(0.0, 0.0), (0.37, 0.91), (-0.2, 1.4), (0.5, 0.5)] {
        assert_eq!(heights.sample(u, v), img.sample_bilinear_wrapped(u, v)[0]);
    }
}

#[test]
fn quality_is_capped() {
    let img = relief(8);
    let at = |quality| occlusion(&img, 0.1, 0.1, quality);
    assert_eq!(at(MAX_QUALITY), at(10_000));
    assert_eq!(at(1), at(0));
}

#[test]
fn it_wraps_and_bakes_the_same_twice() {
    let img = relief(32);
    assert_wraps(&ao(0.1), &img);
    assert_eq!(run(&ao(0.1), &img), run(&ao(0.1), &img));
}

#[test]
fn a_huge_radius_caps_at_half_a_tile_rather_than_panicking() {
    let img = band(16);
    let half = run(&ao(0.5), &img);
    for radius in [f32::INFINITY, 1e17] {
        assert_eq!(run(&ao(radius), &img).pixels(), half.pixels(), "{radius}");
    }
}
