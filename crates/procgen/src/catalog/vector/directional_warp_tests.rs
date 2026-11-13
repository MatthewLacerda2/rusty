//! `directional_warp` moves content along its angle by the map's red, nothing where
//! the map is black, wraps, and reads a bad strength as a no-op or a cap.

use super::*;
use crate::catalog::filter::test_probe::{assert_close, mean, relief, rolled};
use crate::ops::eval_node;
use crate::recipe::OpKind;
use crate::source::Sources;

fn dwarp(strength: f32, angle: f32) -> OpKind {
    OpKind::DirectionalWarp(DirectionalWarp { strength, angle })
}

fn run(op: &OpKind, img: &Image, by: &Image) -> Image {
    eval_node(op, &[img, by], img.size(), 0, &Sources::new())
}

/// A dot one pixel across at `(x, y)` of a 32² tile.
fn dot(x: usize, y: usize) -> Image {
    let mut img = Image::filled(32, [0.0, 0.0, 0.0, 1.0]);
    img.pixels_mut()[y * 32 + x] = [1.0; 4];
    img
}

#[test]
fn white_pushes_the_full_strength_along_the_angle() {
    let white = Image::filled(32, [1.0; 4]);
    // 4 pixels of a 32² tile is 0.125 of it.
    let right = run(&dwarp(0.125, 0.0), &dot(10, 10), &white);
    assert_close(&right, &dot(14, 10), 1e-5, "right");
    let down = run(&dwarp(0.125, 0.25), &dot(10, 10), &white);
    assert_close(&down, &dot(10, 14), 1e-5, "down");
    let back = run(&dwarp(-0.125, 0.0), &dot(10, 10), &white);
    assert_close(&back, &dot(6, 10), 1e-5, "back");
}

#[test]
fn at_an_eighth_turn_it_pushes_along_the_diagonal() {
    let white = Image::filled(32, [1.0; 4]);
    // √2 × 4 pixels along 45° lands 4 right and 4 down.
    let s = 0.125 * std::f32::consts::SQRT_2;
    let out = run(&dwarp(s, 0.125), &dot(10, 10), &white);
    assert_close(&out, &dot(14, 14), 1e-4, "diagonal");
}

#[test]
fn black_moves_nothing() {
    let img = dot(3, 7);
    let black = Image::filled(32, [0.0, 0.0, 0.0, 1.0]);
    assert_eq!(run(&dwarp(0.4, 0.3), &img, &black), img);
}

#[test]
fn it_wraps() {
    let img = Image::fill_uv(48, |u, v| [(u * 6.0).sin() * (v * 3.0).cos(), u, v, 1.0]);
    let by = Image::fill_uv(48, |u, v| [0.5 + 0.5 * (u * 12.6 + v * 6.3).sin(); 4]);
    let op = dwarp(0.2, 0.37);
    let a = run(&op, &rolled(&img, 5, 3), &rolled(&by, 5, 3));
    assert_close(&a, &rolled(&run(&op, &img, &by), 5, 3), 1e-4, "wrap");
}

#[test]
fn a_bad_strength_moves_nothing_or_caps() {
    let (img, white) = (dot(3, 7), Image::filled(32, [1.0; 4]));
    assert_eq!(run(&dwarp(f32::NAN, 0.1), &img, &white), img);
    let cap = run(&dwarp(MAX_STRENGTH, 0.1), &img, &white);
    for strength in [f32::INFINITY, 1e30] {
        assert_eq!(run(&dwarp(strength, 0.1), &img, &white), cap);
    }
    let still = run(&dwarp(0.1, 0.0), &img, &white);
    assert_eq!(run(&dwarp(0.1, f32::INFINITY), &img, &white), still);
    assert_eq!(run(&dwarp(0.1, 1e38), &img, &white), still);
}

#[test]
fn a_wild_map_caps_or_moves_nothing() {
    let img = dot(3, 7);
    let mut wild = Image::filled(32, [1.0; 4]);
    wild.pixels_mut()[5] = [f32::INFINITY; 4];
    wild.pixels_mut()[9] = [f32::NAN; 4];
    wild.pixels_mut()[40] = [1e30; 4];
    for strength in [0.0, 0.3, MAX_STRENGTH] {
        let out = run(&dwarp(strength, 0.1), &img, &wild);
        assert!(out.pixels().iter().flatten().all(|c| c.is_finite()));
    }
    // A red past 1 still caps the push at ±16 tiles.
    let (huge, cap) = (Image::filled(32, [1e6; 4]), Image::filled(32, [1.0; 4]));
    let at = |by: &Image| run(&dwarp(MAX_STRENGTH, 0.1), &img, by);
    assert_eq!(at(&huge), at(&cap));
}

#[test]
fn the_mean_holds_across_resolutions() {
    let at = |n| {
        let by = Image::fill_uv(n, |u, v| {
            [0.5 + 0.5 * (std::f32::consts::TAU * (u + v)).sin(); 4]
        });
        mean(&run(&dwarp(0.1, 0.125), &relief(n), &by), 0)
    };
    let (a, b) = (at(128), at(1024));
    assert!((a - b).abs() < 1e-3, "{a} vs {b}");
}
