//! The staircase search's parts, each asserted on block energies built by hand, where a signal
//! would blur the number under test: the step series' Hann weights and edges, the lags it proposes,
//! a cut's spread and its fewest segments, and how far apart two fits may be and still be one.

use super::{candidates, refined, staircases, steps, Cut, Fit, Levels, SILENT};

fn weights() -> [f64; 4] {
    let (outer, inner) = ((2.0 - 2f64.sqrt()) / 4.0, (2.0 + 2f64.sqrt()) / 4.0);
    [outer, inner, inner, outer]
}

fn side(energy: &[f64]) -> f64 {
    weights().iter().zip(energy).map(|(w, e)| w * e).sum()
}

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

fn exactly_silent() -> [f64; 4] {
    let inner = 0.5 - 0.5 * (std::f64::consts::TAU * 1.5 / 4.0).cos();
    let near = SILENT / inner;
    let x = (-16..=16)
        .map(|ulps: i64| f64::from_bits((near.to_bits() as i64 + ulps) as u64))
        .find(|x| inner * x == SILENT)
        .expect("an energy whose side is exactly SILENT");
    [0.0, x, 0.0, 0.0]
}

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

#[test]
// The echo 13 blocks on repeats under `REPEATS`; 80 and 120 lie past a sixth of 300 blocks.
fn a_candidate_repeats_strongly_within_a_sixth_of_the_stretch() {
    let mut series = vec![0.0; 300];
    for k in (0..300).step_by(40) {
        series[k] = 1.0;
        series[k + 13] = 0.05;
    }
    assert_eq!(candidates(&series), vec![40]);
}

fn segments(levels: &[f64]) -> Levels {
    let energy: Vec<f64> = levels
        .iter()
        .flat_map(|db| [10f64.powf(db / 10.0); 10])
        .collect();
    Levels::of(&energy)
}

#[test]
fn a_cut_takes_six_segments() {
    let level = segments(&[0.0, -1.0, -2.0, -3.0, -4.0, -5.0]);
    assert!(Cut::of(&level, 0, 60, 10.0).is_some());
    assert!(Cut::of(&level, 0, 59, 10.0).is_none());
}

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

#[test]
fn a_fit_continues_one_up_to_two_periods_behind() {
    assert!(fit(0, 100).continues(&fit(140, 300)));
    assert!(!fit(0, 100).continues(&fit(141, 300)));
}

fn staircase(len: usize, first: usize, period: f64) -> Vec<f64> {
    (0..len)
        .map(|k| {
            let n = ((k as f64 - first as f64) / period + 1.0).floor().max(0.0);
            10f64.powf(-n / 10.0)
        })
        .collect()
}

#[test]
// Coarse phases are two blocks apart; steps at blocks 21, 41, ... fall between them.
fn a_phase_between_coarse_ones_is_refined_to() {
    let level = Levels::of(&staircase(800, 21, 20.0));
    let fit = refined(&level, 0, 240, 20).expect("a staircase");
    assert_eq!((fit.first, fit.period), (41, 20.0), "{fit:?}");
    assert!((fit.across + 1.0).abs() < 1e-9, "{fit:?}");
}

#[test]
fn a_staircase_up_to_the_end_of_a_short_file_is_found() {
    let found = staircases(&staircase(700, 442, 20.0), 1);
    assert_eq!(found.len(), 1, "{found:?}");
}
