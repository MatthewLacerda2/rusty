//! A reading is the same however the samples arrive: in runs of any length, to the last frame.

use rusty_zimmer::level::Meter;

use super::{measured, SLACK};

#[test]
fn the_answer_does_not_depend_on_how_the_samples_arrived() {
    let gentle: Vec<f32> = (0..600).map(|i| (i as f32 * 0.1).sin() * 0.6).collect();
    // Both channels alternate every two frames, which is a tone at a quarter of the sample rate
    // sampled on its way past the peak rather than at it.
    let overshooting: Vec<f32> = (0..600)
        .map(|i| if (i / 4) % 2 == 0 { 0.7 } else { -0.7 })
        .collect();

    for signal in [gentle, overshooting] {
        let whole = measured(&signal, 2);
        let mut piecewise = Meter::new(2);
        for run in signal.chunks(64) {
            piecewise.feed(run);
        }
        let split = piecewise.finish();

        assert_eq!(whole.peak_dbfs, split.peak_dbfs);
        assert_eq!(whole.mean_dbfs, split.mean_dbfs);
        let (whole_true, split_true) = (
            whole.true_peak_dbfs.expect("audible"),
            split.true_peak_dbfs.expect("audible"),
        );
        assert!(
            (whole_true - split_true).abs() < SLACK,
            "seams moved the true peak: {whole_true:.3} whole, {split_true:.3} in runs"
        );
    }
}

const IDENTICAL: f64 = 1e-9;

#[test]
fn the_run_length_is_not_in_the_true_peak() {
    let sine: Vec<f32> = (0..96_000)
        .map(|i| (std::f64::consts::TAU * 440.0 * i as f64 / 48_000.0).sin() as f32 * 0.9)
        .collect();

    let whole = measured(&sine, 1).true_peak_dbfs.expect("audible");
    let amplitude = 20.0 * 0.9f64.log10();
    assert!(
        (whole - amplitude).abs() < SLACK,
        "a 0.9 sine peaks at {amplitude:.3} dBTP, and read {whole:.3}"
    );

    for run in [1_024, 4_096, 16_384] {
        let mut meter = Meter::new(1);
        for chunk in sine.chunks(run) {
            meter.feed(chunk);
        }
        let split = meter.finish().true_peak_dbfs.expect("audible");
        assert!(
            (whole - split).abs() < IDENTICAL,
            "runs of {run} read {split:.3} dBTP where the whole signal reads {whole:.3}"
        );
    }
}

#[test]
fn the_last_frames_of_a_signal_are_still_measured() {
    let mut ending = vec![0.1f32; 200];
    ending[199] = 0.95;
    let level = measured(&ending, 1);
    assert!(
        level.true_peak_dbfs.expect("audible") >= level.peak_dbfs.expect("audible"),
        "the closing spike has to be in the reading"
    );
    assert!((level.peak_dbfs.expect("audible") - 20.0 * 0.95f64.log10()).abs() < SLACK);
}
