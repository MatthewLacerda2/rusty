//! Each frame's true peak, as a limiter outside this crate reads it.

use scorsese_zimmer::level::{FramePeaks, Meter};

/// A quarter-rate tone read half a sample late, in stereo with the right side
/// at half level: its waveform overshoots its samples.
fn signal() -> Vec<f32> {
    (0..2_000)
        .flat_map(|i| {
            let s = if (i / 2) % 2 == 0 { 0.7 } else { -0.7 };
            [s, s * 0.5]
        })
        .collect()
}

fn every(samples: &[f32], run: usize) -> Vec<f64> {
    let mut peaks = FramePeaks::new(2);
    let mut read = Vec::new();
    for chunk in samples.chunks(run) {
        peaks.feed(chunk, |peak| read.push(peak));
    }
    peaks.tail(|peak| read.push(peak));
    read
}

/// One reading per frame, whatever the run size, and the same readings.
#[test]
fn every_frame_is_read_once_whatever_the_runs() {
    let signal = signal();
    let whole = every(&signal, signal.len());
    assert_eq!(whole.len(), signal.len() / 2);
    for run in [2, 6, 64, 1_000] {
        assert_eq!(every(&signal, run), whole, "runs of {run}");
    }
}

/// The largest reading is the meter's true peak, louder side first: the two
/// cannot disagree about where the waveform goes.
#[test]
fn the_loudest_frame_is_the_meters_true_peak() {
    let signal = signal();
    let loudest = every(&signal, 100).into_iter().fold(0.0, f64::max);
    let mut meter = Meter::new(2);
    meter.feed(&signal);
    let true_peak = meter.finish().true_peak_dbfs.expect("audible");
    assert!((20.0 * loudest.log10() - true_peak).abs() < 1e-9);
    assert!(loudest > 0.7 * 1.2, "the overshoot was read: {loudest}");
}
