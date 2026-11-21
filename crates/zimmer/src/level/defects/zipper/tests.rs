//! The staircase search's parts, each asserted on block energies built by
//! hand, where a signal would blur the number under test: the step series'
//! Hann weights and edges, the lags it proposes, a cut's spread and its
//! fewest segments, and how far apart two fits may be and still be one.

use super::{Cut, Fit, Levels, SILENT, candidates, refined, staircases, steps};

/// The four Hann weights of a side, `0.5 − 0.5·cos(2π(j + ½)/4)`.
fn weights() -> [f64; 4] {
    let (outer, inner) = ((2.0 - 2f64.sqrt()) / 4.0, (2.0 + 2f64.sqrt()) / 4.0);
    [outer, inner, inner, outer]
}

fn side(energy: &[f64]) -> f64 {
    weights().iter().zip(energy).map(|(w, e)| w * e).sum()
}

/// Each boundary's step is the Hann-weighted four blocks after it over the
/// four before, and nothing within four blocks of either end.
#[test]
fn a_step_is_the_weighted_side_after_over_the_side_before() {
    let energy = [3.0, 1.0, 4.0, 1.0, 5.0, 9.0, 2.0, 6.0, 5.0, 3.0, 5.0, 8.0];
    let found = steps(&energy);
    for (at, step) in found.iter().enumerate() {
        let expected = if (4..=8).contains(&at) {
            10.0 * (side(&energy[at..]) / side(&energy[at - 4..])).log10()
        } else {
            0.0
        };
        assert!((step - expected).abs() < 1e-12, "at {at}: {found:?}");
    }
}

/// An energy whose side is exactly [`SILENT`]: all of it in the second
/// block, the one the weights' order puts it in.
fn exactly_silent() -> [f64; 4] {
    let inner = 0.5 - 0.5 * (std::f64::consts::TAU * 1.5 / 4.0).cos();
    let near = SILENT / inner;
    let x = (-16..=16)
        .map(|ulps: i64| f64::from_bits((near.to_bits() as i64 + ulps) as u64))
        .find(|x| inner * x == SILENT)
        .expect("an energy whose side is exactly SILENT");
    [0.0, x, 0.0, 0.0]
}

/// A side at exactly [`SILENT`] has a level, either side; one under it, on
/// either side, makes the step nothing.
#[test]
fn a_side_has_a_level_from_exactly_silent_up() {
    let silent = exactly_silent();
    let loud = [1.0; 4];
    let step = |before: [f64; 4], after: [f64; 4]| steps(&[before, after].concat())[4];
    let up = step(silent, loud);
    assert!((up - 10.0 * (2.0 / SILENT).log10()).abs() < 1e-9, "{up}");
    let down = step(loud, silent);
    assert!(
        (down - 10.0 * (SILENT / 2.0).log10()).abs() < 1e-9,
        "{down}"
    );
    assert_eq!(step([0.0; 4], loud), 0.0);
    assert_eq!(step(loud, [0.0; 4]), 0.0);
}

/// Steps that repeat every 40 blocks, with a weak echo 13 blocks after each:
/// the echo repeats at 13 and 27 blocks, under 8% of the steps' power, and is
/// no candidate. Nor are 80 and 120, past a sixth of 300 blocks.
#[test]
fn a_candidate_repeats_strongly_within_a_sixth_of_the_stretch() {
    let mut series = vec![0.0; 300];
    for k in (0..300).step_by(40) {
        series[k] = 1.0;
        series[k + 13] = 0.05;
    }
    assert_eq!(candidates(&series), vec![40]);
}

/// Block energies flat across segments of 10 blocks, at `levels` dB.
fn segments(levels: &[f64]) -> Levels {
    let energy: Vec<f64> = levels
        .iter()
        .flat_map(|db| [10f64.powf(db / 10.0); 10])
        .collect();
    Levels::of(&energy)
}

/// A cut takes six segments: six are cut, five are not.
#[test]
fn a_cut_takes_six_segments() {
    let level = segments(&[0.0, -1.0, -2.0, -3.0, -4.0, -5.0]);
    assert!(Cut::of(&level, 0, 60, 10.0).is_some());
    assert!(Cut::of(&level, 0, 59, 10.0).is_none());
}

/// A cut's spread is the sample standard deviation of its steps: steps of
/// 1, 2, 1, 2 and 1 dB down spread by √0.3.
#[test]
fn a_cuts_spread_is_the_sample_deviation_of_its_steps() {
    let level = segments(&[0.0, -1.0, -3.0, -4.0, -6.0, -7.0]);
    let cut = Cut::of(&level, 0, 60, 10.0).expect("six segments");
    assert!((cut.across + 1.4).abs() < 1e-9, "{}", cut.across);
    assert!((cut.spread - 0.3f64.sqrt()).abs() < 1e-9, "{}", cut.spread);
}

fn fit(first: usize, last: usize) -> Fit {
    Fit {
        period: 20.0,
        across: -1.0,
        within: 0.0,
        first,
        last,
        sizes: vec![1.0],
        periods: vec![20.0],
    }
}

/// A fit continues one that ends up to two periods before it starts, and not
/// a block further.
#[test]
fn a_fit_continues_one_up_to_two_periods_behind() {
    assert!(fit(0, 100).continues(&fit(140, 300)));
    assert!(!fit(0, 100).continues(&fit(141, 300)));
}

/// Block energies flat at 0 dB until `first`, then a step of 1 dB down every
/// `period` blocks, to the end of `len` blocks.
fn staircase(len: usize, first: usize, period: f64) -> Vec<f64> {
    (0..len)
        .map(|k| {
            let n = ((k as f64 - first as f64) / period + 1.0).floor().max(0.0);
            10f64.powf(-n / 10.0)
        })
        .collect()
}

/// A lag's phase is refined to a block either side of the best coarse one,
/// which is tried two blocks apart: a staircase stepping one block past a
/// coarse phase is read at its own period and step, not near them.
#[test]
fn a_phase_between_coarse_ones_is_refined_to() {
    let level = Levels::of(&staircase(800, 21, 20.0));
    let fit = refined(&level, 0, 240, 20).expect("a staircase");
    assert_eq!((fit.first, fit.period), (41, 20.0), "{fit:?}");
    assert!((fit.across + 1.0).abs() < 1e-9, "{fit:?}");
}

/// The last stretch also judges a window ending exactly at the end of the
/// file: a staircase over a 700-block file's last 258 blocks is found only
/// there.
#[test]
fn a_staircase_up_to_the_end_of_a_short_file_is_found() {
    let found = staircases(&staircase(700, 442, 20.0), 1);
    assert_eq!(found.len(), 1, "{found:?}");
}
