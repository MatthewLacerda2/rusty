//! Measuring how loud a signal is: peak, true peak, and mean.

use scorsese_zimmer::level::Meter;

use super::SLACK;

fn measured(samples: &[f32], channels: usize) -> scorsese_zimmer::level::Loudness {
    let mut meter = Meter::new(channels);
    meter.feed(samples);
    meter.finish()
}

/// A square wave at full scale sits at 0 dBFS by every measure: every sample is
/// at the rail, so peak and RMS are the same number.
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

/// Halving the amplitude is exactly −6.02 dB, which is the arithmetic this
/// whole report rests on: the two scores that came out "4.5 dB quiet" were
/// diagnosed by comparing numbers like these.
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

/// A silence is reported as a silence, not as minus infinity decibels. It is a
/// clip that makes no sound, which is a different sentence and usually a more
/// urgent one.
#[test]
fn silence_is_said_in_words_rather_than_as_a_number() {
    let level = measured(&[0.0; 500], 1);
    assert!(level.is_silent());
    assert_eq!(level.peak_dbfs, None);
    assert_eq!(level.mean_dbfs, None);
    assert!(!level.is_clipping(), "a silence cannot clip");
    assert!(measured(&[], 1).is_silent(), "and neither can nothing");
}

/// Mean is well below peak for a signal that is mostly quiet — which is the
/// shape of a bed under narration, and the reason both numbers are printed
/// rather than either one alone.
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

/// The case sample peak cannot see: a waveform whose samples all sit below full
/// scale but whose curve between them does not. This is what clips after a
/// lossy encoder, and reporting the sample peak under the name "true peak"
/// would be worse than not reporting it at all.
#[test]
fn true_peak_finds_what_sample_peak_misses() {
    // A half-rate square sampled at its zero crossings: consecutive samples of
    // ±0.7 with the real excursion between them.
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

/// Never below the sample peak: the interpolated curve passes through every
/// sample, so a true peak under one would describe a waveform that does not
/// contain its own samples.
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

/// Fed a run at a time, because a render's mix is written segment by segment
/// and never held whole. The answer must not depend on where the seams fell.
///
/// Asserted on a signal that **has** something between its samples, which a
/// gentle sine does not: the quarter-rate pattern below overshoots its own
/// samples by a decibel, and every left-hand tap of the kernel is carrying
/// part of that. Fed a run at a time with too short a tail the overshoot
/// reads a third of a decibel *high* — truncating the kernel drops
/// negatively-weighted taps, so a seam inflates rather than flattens — and on
/// a sine the same mistake moves the answer by a hundredth of a decibel and
/// nothing notices. The signal is what makes this test about the tail.
#[test]
fn the_answer_does_not_depend_on_how_the_samples_arrived() {
    let gentle: Vec<f32> = (0..600).map(|i| (i as f32 * 0.1).sin() * 0.6).collect();
    // Both channels alternate every two frames, which is a tone at a quarter
    // of the sample rate sampled on its way past the peak rather than at it.
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
        // True peak is the one that could differ, since the interpolator looks
        // either side of a sample — which is why the meter keeps the tail of
        // the previous run rather than treating every seam as an edge.
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

/// Decibels of slack for two readings that must be the *same* reading rather
/// than merely a close one. Once no frame is measured against a fabricated
/// edge, every frame is measured exactly once against its real neighbours
/// whatever the run length was — the same arithmetic on the same samples in
/// the same order — so the two answers agree to the last bit, and a tolerance
/// this far below the kernel's own error is a way of saying so.
const IDENTICAL: f64 = 1e-9;

/// **The run length must not be in the answer**, and it was: every run
/// boundary used to be reconstructed as an edge in silence, and the ringing
/// off that fabricated edge was counted as an intersample peak. A 440 Hz sine
/// at 0.9 read −0.894 dBTP fed whole and +0.078 fed in 4 KB runs — 0.97 dB of
/// pure artefact, enough on its own to call a file at −1 dBTP clipping.
///
/// A sine at a tenth of the sample rate is the signal that makes this visible:
/// it has essentially nothing between its samples, so the right answer is its
/// own amplitude and *everything* above that is the seam. The absolute
/// assertion is the one that matters — two wrong readings that agree would
/// satisfy the comparison on its own.
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

/// The end of the signal is the one place the silence either side of a run is
/// real, so the frames held back for their right-hand neighbours must still be
/// measured when nobody feeds any more. A spike in the last few frames is what
/// goes missing if they are not.
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
