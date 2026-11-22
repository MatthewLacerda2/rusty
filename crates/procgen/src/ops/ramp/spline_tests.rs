//! `color_ramp`'s `cardinal` and `b_spline` fill against the curve each is meant to be.

use super::super::super::recipe::{RampInterpolation, RampStop};
use super::sample_ramp;
use super::spline::{weights, Spline};

const CARDINAL: RampInterpolation = RampInterpolation::Cardinal;
const B_SPLINE: RampInterpolation = RampInterpolation::BSpline;

fn stops(points: &[(f32, f32)]) -> Vec<RampStop> {
    let to_stop = |&(pos, v): &(f32, f32)| RampStop {
        pos,
        color: [v, v, v, 1.0],
    };
    points.iter().map(to_stop).collect()
}

fn at(sorted: &[RampStop], kind: RampInterpolation, t: f32) -> f32 {
    sample_ramp(sorted, kind, t)[0]
}

fn sweep(sorted: &[RampStop], kind: RampInterpolation) -> Vec<f32> {
    (0..=1000)
        .map(|i| at(sorted, kind, i as f32 / 1000.0))
        .collect()
}

/// A curve through five evenly spaced stops, none near 0 or 1 (where the clamp would
/// bend it).
fn profile() -> Vec<RampStop> {
    stops(&[
        (0.0, 0.3),
        (0.25, 0.7),
        (0.5, 0.4),
        (0.75, 0.6),
        (1.0, 0.35),
    ])
}

#[test]
fn the_weights_are_blenders_and_sum_to_one() {
    for kind in [Spline::Cardinal, Spline::BSpline] {
        for i in 0..=20 {
            let sum: f32 = weights(i as f32 / 20.0, kind).iter().sum();
            assert!((sum - 1.0).abs() < 1e-5, "{kind:?} at {i}/20: {sum}");
        }
    }
    // At the segment's middle, by hand from `key_curve_position_weights`: cardinal
    // (fc = 0.71) is [-fc/8, 1/2 + fc/8, 1/2 + fc/8, -fc/8]; B-spline [1, 23, 23, 1]/48.
    let close = |a: [f32; 4], b: [f32; 4]| a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-6);
    let fc8 = 0.71 / 8.0;
    assert!(close(
        weights(0.5, Spline::Cardinal),
        [-fc8, 0.5 + fc8, 0.5 + fc8, -fc8]
    ));
    let b = [1.0 / 48.0, 23.0 / 48.0, 23.0 / 48.0, 1.0 / 48.0];
    assert!(
        close(weights(0.5, Spline::BSpline), b),
        "{:?}",
        weights(0.5, Spline::BSpline)
    );
    assert_eq!(weights(0.0, Spline::Cardinal), [0.0, 1.0, 0.0, 0.0]);
}

#[test]
fn cardinal_passes_through_every_stop() {
    let ramp = profile();
    for s in &ramp {
        let got = at(&ramp, CARDINAL, s.pos);
        assert!((got - s.color[0]).abs() < 1e-5, "{}: {got}", s.pos);
    }
}

#[test]
fn cardinal_has_no_corner_at_a_stop() {
    let ramp = profile();
    let h = 1e-4;
    let slopes = |kind, p: f32| {
        let (l, m, r) = (
            at(&ramp, kind, p - h),
            at(&ramp, kind, p),
            at(&ramp, kind, p + h),
        );
        ((m - l) / h, (r - m) / h)
    };
    for p in [0.25, 0.5, 0.75] {
        let (left, right) = slopes(CARDINAL, p);
        assert!((left - right).abs() < 0.01, "{p}: {left} vs {right}");
        let (left, right) = slopes(RampInterpolation::Linear, p);
        assert!(
            (left - right).abs() > 0.5,
            "linear at {p}: {left} vs {right}"
        );
    }
}

#[test]
fn b_spline_stays_inside_the_stops_where_cardinal_overshoots() {
    let ramp = stops(&[(0.0, 0.1), (0.25, 0.1), (0.5, 0.8), (0.75, 0.8), (1.0, 0.1)]);
    let max = |v: Vec<f32>| v.into_iter().fold(f32::MIN, f32::max);
    let min = |v: Vec<f32>| v.into_iter().fold(f32::MAX, f32::min);
    assert!(max(sweep(&ramp, CARDINAL)) > 0.85);
    assert!(min(sweep(&ramp, CARDINAL)) < 0.09);
    assert!(max(sweep(&ramp, B_SPLINE)) <= 0.8 + 1e-6);
    assert!(min(sweep(&ramp, B_SPLINE)) >= 0.1 - 1e-6);
    // B-spline meets no inner stop: it rounds the plateau's corners off.
    assert!(at(&ramp, B_SPLINE, 0.5) < 0.8 - 0.05);
}

#[test]
fn a_straight_ramp_stays_straight() {
    let flat = stops(&[(0.2, 0.6), (0.7, 0.6)]);
    for kind in [CARDINAL, B_SPLINE] {
        assert!(sweep(&flat, kind).iter().all(|v| (v - 0.6).abs() < 1e-6));
    }
    let line = (0..=8).map(|i| (i as f32 / 8.0, 0.1 + 0.1 * i as f32));
    let line = stops(&line.collect::<Vec<_>>());
    for i in 250..=750 {
        let t = i as f32 / 1000.0;
        let (got, want) = (
            at(&line, B_SPLINE, t),
            at(&line, RampInterpolation::Linear, t),
        );
        assert!((got - want).abs() < 1e-5, "{t}: {got} vs {want}");
    }
}

#[test]
fn the_ends_follow_blender() {
    let ramp = stops(&[(0.3, 0.2), (0.5, 0.8), (0.7, 0.4)]);
    let before = sweep(&ramp, CARDINAL)[..300].to_vec();
    assert!(before.iter().any(|&v| (v - 0.2).abs() > 0.01), "{before:?}");
    assert!((at(&ramp, CARDINAL, 0.0) - 0.2).abs() < 1e-6);
    assert!((at(&ramp, CARDINAL, 1.0) - 0.4).abs() < 1e-6);
    let one = stops(&[(0.4, 0.7)]);
    for kind in [CARDINAL, B_SPLINE] {
        assert!(sweep(&one, kind).iter().all(|&v| v == 0.7));
    }
}
