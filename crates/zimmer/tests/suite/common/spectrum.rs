use std::f64::consts::TAU;

use rusty_zimmer::SAMPLE_RATE;

/// Samples per block: 0.19 s at 44.1 kHz, so bins sit 5.4 Hz apart.
const BLOCK: usize = 8192;

/// Frequencies per octave centre, ±6% around it: one noise bin alone is all error.
const PROBES: usize = 12;

/// Hann-windowed: brown noise spans 40 dB, and a rectangular window's leakage would flatten its
/// slope.
pub(crate) fn band_power(buf: &[f32], centre: f32) -> f64 {
    let mut total = 0.0;
    let mut count = 0.0;
    for block in buf.chunks_exact(BLOCK) {
        for probe in 0..PROBES {
            let hz = f64::from(centre) * (0.94 + 0.12 * probe as f64 / (PROBES - 1) as f64);
            let (mut re, mut im) = (0.0, 0.0);
            // The phasor advances by a complex multiply rather than a `sin` per sample: same
            // numbers, a fraction of the arithmetic.
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
    if count > 0.0 {
        total / count
    } else {
        0.0
    }
}

pub(crate) fn centres(lowest: f32, octaves: usize) -> Vec<f32> {
    (0..octaves).map(|k| lowest * (1 << k) as f32).collect()
}

pub(crate) fn octave_db(buf: &[f32], lowest: f32, octaves: usize) -> Vec<f64> {
    centres(lowest, octaves)
        .into_iter()
        .map(|hz| 10.0 * band_power(buf, hz).log10())
        .collect()
}

/// A least-squares fit, not two bands: two bands cannot tell a straight slope from flat-then-cliff.
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
