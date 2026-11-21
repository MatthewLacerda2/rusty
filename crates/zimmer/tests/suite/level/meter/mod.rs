//! Measuring how loud a signal is: peak, true peak, and mean.

use rusty_zimmer::level::Meter;

use super::SLACK;

mod runs;

fn measured(samples: &[f32], channels: usize) -> rusty_zimmer::level::Loudness {
    let mut meter = Meter::new(channels);
    meter.feed(samples);
    meter.finish()
}

#[test]
fn a_full_scale_square_reads_as_full_scale() {
    let square: Vec<f32> = (0..1000)
        .map(|i| if i % 2 == 0 { 1.0 } else { -1.0 })
        .collect();
    let level = measured(&square, 1);
    assert!((level.peak_dbfs.expect("audible") - 0.0).abs() < SLACK);
    assert!((level.mean_dbfs.expect("audible") - 0.0).abs() < SLACK);
    assert!(level.is_clipping(), "at the rail is where clipping begins");
}

#[test]
fn halving_the_amplitude_is_six_decibels_down() {
    let half: Vec<f32> = (0..1000)
        .map(|i| if i % 2 == 0 { 0.5 } else { -0.5 })
        .collect();
    let level = measured(&half, 1);
    assert!((level.peak_dbfs.expect("audible") + 6.02).abs() < SLACK);
    assert!((level.mean_dbfs.expect("audible") + 6.02).abs() < SLACK);
    assert!(!level.is_clipping());
}

#[test]
fn silence_is_said_in_words_rather_than_as_a_number() {
    let level = measured(&[0.0; 500], 1);
    assert!(level.is_silent());
    assert_eq!(level.peak_dbfs, None);
    assert_eq!(level.mean_dbfs, None);
    assert!(!level.is_clipping(), "a silence cannot clip");
    assert!(measured(&[], 1).is_silent(), "and neither can nothing");
}

#[test]
fn a_mostly_quiet_signal_reads_quiet_in_the_mean_and_loud_in_the_peak() {
    let mut sparse = vec![0.0f32; 1000];
    sparse[500] = 1.0;
    let level = measured(&sparse, 1);
    assert!((level.peak_dbfs.expect("audible") - 0.0).abs() < SLACK);
    assert!(
        level.mean_dbfs.expect("audible") < -25.0,
        "one full-scale sample in a thousand is a very quiet signal on average"
    );
}

#[test]
fn true_peak_finds_what_sample_peak_misses() {
    // A half-rate square sampled at its zero crossings: consecutive samples of ±0.7 with the real
    // excursion between them.
    let between: Vec<f32> = (0..400)
        .map(|i| if (i / 2) % 2 == 0 { 0.7 } else { -0.7 })
        .collect();
    let level = measured(&between, 1);
    let (peak, true_peak) = (
        level.peak_dbfs.expect("audible"),
        level.true_peak_dbfs.expect("audible"),
    );
    assert!(
        true_peak > peak,
        "the curve between samples must read above the samples: \
         peak {peak:.2} dBFS, true peak {true_peak:.2} dBFS"
    );
}

#[test]
fn true_peak_is_never_under_the_sample_peak() {
    for signal in [
        vec![1.0f32; 200],
        vec![0.25; 200],
        vec![0.0, 0.9, 0.0, -0.9],
    ] {
        let level = measured(&signal, 1);
        assert!(level.true_peak_dbfs >= level.peak_dbfs);
    }
}
