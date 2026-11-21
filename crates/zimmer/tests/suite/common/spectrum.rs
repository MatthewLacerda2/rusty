//! How much energy a signal has, octave by octave, and what line that traces.
//!
//! There is no FFT in this workspace and this is not one: it correlates the
//! signal against a handful of analysis frequencies around each octave centre
//! and averages, which is Welch's method with the bins picked in advance. That
//! is all a *slope* needs — the question is how the energy falls across seven
//! octaves, not what is at any one frequency — and it costs forty lines rather
//! than a dependency.
//!
//! Noise is stochastic, so one estimate at one frequency is worthless: a
//! single periodogram bin has 100% relative error however long the buffer.
//! Averaging over [`PROBES`] frequencies and every block in the signal is what
//! turns it into a number worth asserting on, and the least-squares fit over
//! several octaves averages once more.

use std::f64::consts::TAU;

use scorsese_zimmer::SAMPLE_RATE;

/// Samples per analysis block. At 44.1 kHz this is a fifth of a second, whose
/// bins are 5 Hz apart — fine enough that the lowest octave centre measured is
/// still a dozen bins up from DC.
const BLOCK: usize = 8192;

/// Analysis frequencies per octave centre, spread ±6% around it. Independent
/// estimates of the same band: the average of them is what has an error bar
/// small enough to fit a line through.
const PROBES: usize = 12;

/// Mean power at `centre` Hz, over every whole block of `buf`.
///
/// Hann-windowed, which matters more here than usual: brown noise puts 40 dB
/// between its bottom octave and its top, and a rectangular window's spectral
/// leakage would carry the bottom into the measurement of the top and report a
/// slope far flatter than the signal has.
pub(crate) fn band_power(buf: &[f32], centre: f32) -> f64 {
    let mut total = 0.0;
    let mut count = 0.0;
    for block in buf.chunks_exact(BLOCK) {
        for probe in 0..PROBES {
            let hz = f64::from(centre) * (0.94 + 0.12 * probe as f64 / (PROBES - 1) as f64);
            let (mut re, mut im) = (0.0, 0.0);
            // The phasor advances by a complex multiply rather than a `sin`
            // per sample: same numbers, a fraction of the arithmetic.
            let step = TAU * hz / f64::from(SAMPLE_RATE);
            let (cos_step, sin_step) = (step.cos(), step.sin());
            let (mut cos, mut sin) = (1.0, 0.0);
            for (n, sample) in block.iter().enumerate() {
                let window = 0.5 - 0.5 * (TAU * n as f64 / BLOCK as f64).cos();
                let value = window * f64::from(*sample);
                re += value * cos;
                im += value * sin;
                (cos, sin) = (
                    cos * cos_step - sin * sin_step,
                    sin * cos_step + cos * sin_step,
                );
            }
            total += re * re + im * im;
            count += 1.0;
        }
    }
    if count > 0.0 { total / count } else { 0.0 }
}

/// The octave centres a measurement runs over: `octaves` of them, doubling
/// from `lowest` Hz.
pub(crate) fn centres(lowest: f32, octaves: usize) -> Vec<f32> {
    (0..octaves).map(|k| lowest * (1 << k) as f32).collect()
}

/// The band powers at those centres, in decibels.
pub(crate) fn octave_db(buf: &[f32], lowest: f32, octaves: usize) -> Vec<f64> {
    centres(lowest, octaves)
        .into_iter()
        .map(|hz| 10.0 * band_power(buf, hz).log10())
        .collect()
}

/// Decibels per octave: the least-squares slope of [`octave_db`] against the
/// log of frequency.
///
/// A fit rather than a difference between two bands, because two bands cannot
/// tell a straight −3 dB line from a filter that is flat and then falls off a
/// cliff — which is exactly the distinction a colour is.
pub(crate) fn slope_db_per_octave(buf: &[f32], lowest: f32, octaves: usize) -> f64 {
    let points: Vec<(f64, f64)> = octave_db(buf, lowest, octaves)
        .into_iter()
        .enumerate()
        .map(|(at, db)| (at as f64, db))
        .collect();
    let n = points.len() as f64;
    let mean_x = points.iter().map(|p| p.0).sum::<f64>() / n;
    let mean_y = points.iter().map(|p| p.1).sum::<f64>() / n;
    let covariance: f64 = points.iter().map(|p| (p.0 - mean_x) * (p.1 - mean_y)).sum();
    let spread: f64 = points.iter().map(|p| (p.0 - mean_x).powi(2)).sum();
    covariance / spread
}
