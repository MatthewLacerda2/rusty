//! The ops with a reach in pixels, on a non-square image (#1493): each axis takes
//! its own pixel count, checked against analytic values.

use std::f32::consts::TAU;

use super::super::recipe::BlurKind;
use super::*;
use crate::procgen::hash;

/// 64 across, 16 down: a pixel down is four pixels across in tile units.
const WIDE: Size = Size::new(64, 16);

fn gray(v: f32) -> [f32; 4] {
    [v, v, v, 1.0]
}

#[test]
fn a_blur_radius_is_a_fraction_of_each_axis() {
    // 0.125 of the tile is 8 px across but 2 px down.
    assert_eq!(filter::pixel_radii(0.125, WIDE), (8.0, 2.0));
    // Horizontal stripes (varying down only): a box reaching 2 px down is the
    // mean of five wrapped rows; the 8 px across changes nothing.
    let img = Image::fill_uv(WIDE, |_, v| gray((v * 16.0).floor()));
    let out = filter::blur(&img, filter::pixel_radii(0.125, WIDE), BlurKind::Box);
    for y in 0..16i64 {
        let mean = (-2..=2).map(|k| (y + k).rem_euclid(16) as f32).sum::<f32>() / 5.0;
        for x in 0..64 {
            assert!((out.get_wrapped(x, y)[0] - mean).abs() < 1e-5, "({x}, {y})");
        }
    }
}

#[test]
fn bump_to_normal_slopes_in_tile_units_on_each_axis() {
    // h = A sin(2πu) + B sin(2πv): the tile-unit slope is 2πA cos(2πu) across and
    // 2πB cos(2πv) down, whatever the pixel count of either axis. A central
    // difference over n px reads it times sin(2π/n) / (2π/n), exactly.
    let sinc = |n: f32| (TAU / n).sin() / (TAU / n);
    let (a, b, strength) = (0.05, 0.03, 1.0);
    let img = Image::fill_uv(WIDE, |u, v| gray(a * (TAU * u).sin() + b * (TAU * v).sin()));
    let out = vector::bump_to_normal(&img, strength);
    for (i, p) in out.pixels().iter().enumerate() {
        let (x, y) = (i % 64, i / 64);
        let (u, v) = ((x as f32 + 0.5) / 64.0, (y as f32 + 0.5) / 16.0);
        let dx = TAU * a * (TAU * u).cos() * sinc(64.0);
        let dy = TAU * b * (TAU * v).cos() * sinc(16.0);
        let inv = 1.0 / (dx * dx + dy * dy + 1.0).sqrt();
        let (nx, ny) = (-dx * inv * 0.5 + 0.5, -dy * inv * 0.5 + 0.5);
        assert!((p[0] - nx).abs() < 1e-4, "({x}, {y}) x: {} vs {nx}", p[0]);
        assert!((p[1] - ny).abs() < 1e-4, "({x}, {y}) y: {} vs {ny}", p[1]);
    }
}

/// The value [`distance::distance`] gives a pixel `d` tile units from the nearest
/// outside pixel's centre, at `radius`: half a 64-px pixel short, over the radius.
fn expected(d: f32, radius: f32) -> f32 {
    ((d * 64.0 - 0.5) / (radius * 64.0)).clamp(0.0, 1.0)
}

#[test]
fn distance_is_measured_in_tile_units_on_each_axis() {
    let radius = 0.25;
    // A band across (rows 4..12): the nearest outside rows are 3 and 12, each a
    // sixteenth of the tile per row.
    let rows = Image::fill_uv(WIDE, |_, v| gray(f32::from((0.25..0.75).contains(&v))));
    let out = distance::distance(&rows, radius);
    for y in 0..16i64 {
        let want = match y {
            4..12 => expected((y - 3).min(12 - y) as f32 / 16.0, radius),
            _ => 0.0,
        };
        assert!((out.get_wrapped(7, y)[0] - want).abs() < 1e-5, "row {y}");
    }
    // The same band of the tile down (columns 16..48), a sixty-fourth per column.
    let cols = Image::fill_uv(WIDE, |u, _| gray(f32::from((0.25..0.75).contains(&u))));
    let out = distance::distance(&cols, radius);
    for x in 0..64i64 {
        let want = match x {
            16..48 => expected((x - 15).min(48 - x) as f32 / 64.0, radius),
            _ => 0.0,
        };
        assert!((out.get_wrapped(x, 5)[0] - want).abs() < 1e-5, "column {x}");
    }
}

#[test]
fn white_noise_grain_is_one_pixel_on_each_axis() {
    let img = Image::fill_uv(WIDE, generators::white_noise(WIDE, 7));
    for (x, y) in [(0, 0), (63, 0), (5, 15), (40, 9)] {
        assert_eq!(
            img.get_wrapped(x, y)[0],
            hash::unit2(x, y, 0, 7),
            "({x}, {y})"
        );
    }
}
