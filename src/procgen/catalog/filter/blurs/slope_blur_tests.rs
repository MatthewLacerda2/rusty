//! `slope_blur` runs an image down another's slopes, leaves flat ground alone,
//! wraps, holds across resolutions and reads bad numbers as a no-op or a cap.

use super::*;
use crate::procgen::catalog::filter::test_probe::{assert_close, mean, relief, ridged, rolled};
use crate::procgen::ops::eval_node;
use crate::procgen::recipe::OpKind;
use crate::procgen::source::Sources;

fn sblur(intensity: f32, samples: u32, mode: SlopeMode) -> OpKind {
    OpKind::SlopeBlur(SlopeBlur {
        intensity,
        samples,
        mode,
    })
}

fn run(op: &OpKind, img: &Image, slope: &Image) -> Image {
    eval_node(op, &[img, slope], img.size(), 0, &Sources::new())
}

/// A height rising left to right over the tile's first quarter: `0.5 + 0.5 sin`.
fn hill(n: u32) -> Image {
    Image::fill_uv(n, |u, _| [0.5 + 0.5 * (std::f32::consts::TAU * u).sin(); 4])
}

#[test]
fn flat_ground_is_left_as_it_is() {
    let img = relief(16);
    let flat = Image::filled(16, [0.3; 4]);
    for mode in [SlopeMode::Blur, SlopeMode::Min, SlopeMode::Max] {
        assert!(run(&sblur(0.5, 8, mode), &img, &flat) == img, "{mode:?}");
    }
}

#[test]
fn a_dot_runs_down_the_slope_not_up_it() {
    let mut dot = Image::filled(64, [0.0, 0.0, 0.0, 1.0]);
    dot.pixels_mut()[4] = [1.0; 4];
    let out = run(&sblur(0.05, 16, SlopeMode::Max), &dot, &hill(64));
    let row = out.row(0);
    assert!(row[1][0] > 0.5, "downhill of the dot: {:?}", &row[..10]);
    assert_eq!(row[7][0], 0.0, "uphill of the dot");
}

#[test]
fn a_noise_on_a_slope_smears_along_it() {
    let out = run(&sblur(0.05, 16, SlopeMode::Blur), &ridged(64), &hill(64));
    let step = |dx, dy| {
        let mut s = 0.0;
        for y in 0..64 {
            for x in 0..12 {
                let d = out.get_wrapped(x + dx, y + dy)[0] - out.get_wrapped(x, y)[0];
                s += d * d;
            }
        }
        s
    };
    // Where the hill is steepest, the smear runs across.
    assert!(
        step(1, 0) * 2.0 < step(0, 1),
        "{} vs {}",
        step(1, 0),
        step(0, 1)
    );
}

#[test]
fn min_and_max_bound_the_blur() {
    let (img, slope) = (ridged(32), relief(32));
    let at = |mode| run(&sblur(0.02, 8, mode), &img, &slope);
    let (lo, mid, hi) = (at(SlopeMode::Min), at(SlopeMode::Blur), at(SlopeMode::Max));
    for ((a, b), c) in lo.pixels().iter().zip(mid.pixels()).zip(hi.pixels()) {
        assert!(
            a[0] <= b[0] + 1e-6 && b[0] <= c[0] + 1e-6,
            "{a:?} {b:?} {c:?}"
        );
    }
}

#[test]
fn it_wraps() {
    let (img, slope) = (ridged(48), relief(48));
    let op = sblur(0.03, 8, SlopeMode::Blur);
    let a = run(&op, &rolled(&img, 5, 3), &rolled(&slope, 5, 3));
    assert_close(&a, &rolled(&run(&op, &img, &slope), 5, 3), 1e-4, "wrap");
}

#[test]
fn the_mean_holds_across_resolutions() {
    let at = |n| {
        mean(
            &run(&sblur(0.02, 8, SlopeMode::Blur), &relief(n), &hill(n)),
            0,
        )
    };
    let (a, b) = (at(128), at(1024));
    assert!((a - b).abs() < 2e-3, "{a} vs {b}");
}

#[test]
fn bad_numbers_pass_through_or_cap() {
    let (img, slope) = (relief(16), hill(16));
    let at = |i, n| run(&sblur(i, n, SlopeMode::Blur), &img, &slope);
    assert_eq!(at(f32::NAN, 8), img);
    assert_eq!(at(f32::INFINITY, 8), at(MAX_INTENSITY, 8));
    assert_eq!(at(1e30, 8), at(MAX_INTENSITY, 8));
    assert_eq!(at(0.1, 0), at(0.1, 1));
    assert_eq!(at(0.1, u32::MAX), at(0.1, MAX_SAMPLES));
    let mut wild = slope.clone();
    wild.pixels_mut()[3] = [f32::INFINITY; 4];
    assert!(run(&sblur(0.1, 8, SlopeMode::Blur), &img, &wild)
        .pixels()
        .iter()
        .all(|p| p.iter().all(|c| c.is_finite())));
}

#[test]
fn every_mode_reads_the_pixel_itself() {
    let mut dot = Image::filled(64, [0.0, 0.0, 0.0, 1.0]);
    dot.pixels_mut()[4] = [1.0; 4];
    for n in [1, 8] {
        let max = run(&sblur(0.05, n, SlopeMode::Max), &dot, &hill(64));
        assert_eq!(max.row(0)[4][0], 1.0, "a bright dot stays lit at {n}");
        let min = run(&sblur(0.05, n, SlopeMode::Min), &dot, &hill(64));
        assert_eq!(min.row(0)[4][0], 0.0, "{n}");
    }
}

#[test]
fn a_hard_edge_reaches_as_far_at_every_resolution() {
    // A step up over u ∈ [0.25, 0.75) on a ramp: what the smear moves, per tile.
    let moved = |n| {
        let step = Image::fill_uv(n, |u, _| [f32::from((0.25..0.75).contains(&u)); 4]);
        let ramp = Image::fill_uv(n, |u, _| [u, 0.0, 0.0, 1.0]);
        let out = run(&sblur(0.005, 8, SlopeMode::Blur), &ramp, &step);
        let d = out.pixels().iter().zip(ramp.pixels());
        d.map(|(p, q)| (p[0] - q[0]).abs()).sum::<f32>() / (n * n) as f32
    };
    let (a, b) = (moved(128), moved(1024));
    assert!(a > 1e-3 && (a - b).abs() < 0.05 * a, "{a} vs {b}");
}
