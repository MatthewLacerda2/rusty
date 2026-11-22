//! `color_ramp`'s `linear`, `constant` and `ease` fill, and the binary search every
//! fill shares.

use super::*;

fn stop(pos: f32, v: f32) -> RampStop {
    RampStop {
        pos,
        color: [v, v, v, 1.0],
    }
}

fn ramp(interpolation: RampInterpolation) -> Vec<f32> {
    let stops = [stop(0.8, 1.0), stop(0.2, 0.0), stop(0.5, 0.5)];
    let img = Image::fill_uv(64, |u, _| [u, 0.0, 0.0, 1.0]);
    let out = color_ramp(img, &stops, interpolation);
    out.row(0).iter().map(|p| p[0]).collect()
}

#[test]
fn constant_holds_each_stop_up_to_the_next() {
    let out = ramp(RampInterpolation::Constant);
    assert!(out.iter().all(|v| [0.0, 0.5, 1.0].contains(v)), "{out:?}");
    assert_eq!(
        sample_ramp(
            &[stop(0.2, 0.0), stop(0.5, 0.5)],
            RampInterpolation::Constant,
            0.5
        )[0],
        0.5
    );
    assert_eq!(
        sample_ramp(&[stop(0.2, 0.3)], RampInterpolation::Constant, 0.1)[0],
        0.3
    );
}

#[test]
fn ease_meets_linear_at_stops_and_midpoints_and_bends_between() {
    let sorted = [stop(0.2, 0.0), stop(0.6, 1.0)];
    let at = |i, t| sample_ramp(&sorted, i, t)[0];
    for t in [0.0, 0.2, 0.4, 0.6, 1.0] {
        let (lin, ease) = (
            at(RampInterpolation::Linear, t),
            at(RampInterpolation::Ease, t),
        );
        assert!((lin - ease).abs() < 1e-6, "t={t}: {lin} vs {ease}");
    }
    // A quarter of the way: linear 0.25, smoothstep 0.15625.
    assert!((at(RampInterpolation::Linear, 0.3) - 0.25).abs() < 1e-6);
    assert!((at(RampInterpolation::Ease, 0.3) - 0.15625).abs() < 1e-6);
}

#[test]
fn the_search_agrees_with_the_walk() {
    let stops: Vec<RampStop> = [0.0, 0.1, 0.1, 0.1, 0.4, 0.4, 0.75, 1.0, 1.0]
        .iter()
        .enumerate()
        .map(|(i, &pos)| stop(pos, i as f32 / 8.0))
        .collect();
    let mut ts: Vec<f32> = (-10..=110).map(|i| i as f32 / 100.0).collect();
    ts.extend([0.1, 0.4, f32::NAN, f32::INFINITY, -f32::INFINITY]);
    for interp in [
        RampInterpolation::Constant,
        RampInterpolation::Linear,
        RampInterpolation::Ease,
        RampInterpolation::Cardinal,
        RampInterpolation::BSpline,
    ] {
        for n in 1..=stops.len() {
            for &t in &ts {
                let (got, want) = (
                    sample_ramp(&stops[..n], interp, t),
                    scan_ramp(&stops[..n], interp, t),
                );
                assert_eq!(got.map(f32::to_bits), want.map(f32::to_bits), "{n} {t}");
            }
        }
    }
}

/// The work a large lookup ramp costs is a search, not a walk:
/// a 4096-stop ramp over 4096 pixels reads a few dozen stops a pixel, where the
/// walk (or a per-pixel NaN check) reads thousands.
#[test]
fn a_large_ramp_reads_logarithmically_many_stops_a_pixel() {
    let n = 4096;
    let stops: Vec<RampStop> = (0..n).map(|i| stop(i as f32 / n as f32, 0.5)).collect();
    for interp in [
        RampInterpolation::Constant,
        RampInterpolation::Linear,
        RampInterpolation::Ease,
        RampInterpolation::Cardinal,
        RampInterpolation::BSpline,
    ] {
        let img = Image::fill_uv(64, |u, v| [(u + v) * 0.5, 0.0, 0.0, 1.0]);
        PROBES.with(|p| p.set(0));
        let _ = color_ramp(img, &stops, interp);
        let reads = PROBES.with(std::cell::Cell::get);
        let pixels = 64 * 64;
        // One NaN pass over the stops, then two searches of ~13 reads a pixel.
        assert!(reads < n + pixels * 40, "{interp:?}: {reads} stop reads");
    }
}
