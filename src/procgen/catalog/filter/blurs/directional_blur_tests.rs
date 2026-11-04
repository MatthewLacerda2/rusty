//! `directional_blur` streaks along its angle, at 45° too, wraps, keeps the mean
//! across resolutions, and reads a bad length as a no-op or a cap.

use super::*;
use crate::procgen::catalog::filter::test_probe::{assert_wraps, mean, relief, ridged, run};
use crate::procgen::recipe::OpKind;

fn dblur(length: f32, angle: f32) -> OpKind {
    OpKind::DirectionalBlur(DirectionalBlur { length, angle })
}

/// The mean squared step between pixel `(x, y)` and `(x + dx, y + dy)`.
fn roughness(img: &Image, dx: i64, dy: i64) -> f32 {
    let (w, h) = (img.width() as i64, img.height() as i64);
    let mut sum = 0.0;
    for y in 0..h {
        for x in 0..w {
            let d = img.get_wrapped(x + dx, y + dy)[0] - img.get_wrapped(x, y)[0];
            sum += d * d;
        }
    }
    sum / (w * h) as f32
}

#[test]
fn a_ridged_noise_at_an_eighth_turn_streaks_along_the_diagonal() {
    let out = run(&dblur(0.2, 0.125), &ridged(128));
    // Smooth along the 45° line (down-right), rough across it (down-left).
    let (along, across) = (roughness(&out, 1, 1), roughness(&out, -1, 1));
    assert!(along * 4.0 < across, "along {along} vs across {across}");
}

#[test]
fn angle_zero_streaks_across_and_a_quarter_turn_down() {
    let img = ridged(64);
    let flat = run(&dblur(0.2, 0.0), &img);
    let down = run(&dblur(0.2, 0.25), &img);
    assert!(roughness(&flat, 1, 0) * 4.0 < roughness(&flat, 0, 1));
    assert!(roughness(&down, 0, 1) * 4.0 < roughness(&down, 1, 0));
}

#[test]
fn it_wraps_at_any_angle_and_in_two_passes() {
    for (length, angle) in [(0.05, 0.125), (0.3, 0.37), (0.9, 0.6)] {
        assert_wraps(&dblur(length, angle), &relief(96));
    }
}

#[test]
fn the_mean_holds_across_resolutions() {
    for n in [128, 1024] {
        let img = relief(n);
        let out = run(&dblur(0.3, 0.125), &img);
        let (a, b) = (mean(&img, 0), mean(&out, 0));
        assert!((a - b).abs() < 1e-3, "{n}²: {a} vs {b}");
    }
}

#[test]
fn a_sub_pixel_length_still_softens() {
    let img = relief(16);
    let out = run(&dblur(0.01, 0.0), &img);
    assert_ne!(out, img);
}

#[test]
fn a_bad_length_passes_through_or_caps_at_a_tile() {
    let img = relief(16);
    for length in [0.0, -1.0, f32::NAN] {
        assert_eq!(run(&dblur(length, 0.1), &img), img, "{length}");
    }
    let tile = run(&dblur(1.0, 0.1), &img);
    for length in [f32::INFINITY, 1e30] {
        assert_eq!(run(&dblur(length, 0.1), &img), tile, "{length}");
    }
    assert_eq!(
        run(&dblur(0.1, f32::NAN), &img),
        run(&dblur(0.1, 0.0), &img)
    );
}
