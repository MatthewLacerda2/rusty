//! How loud a signal came out: peak, true peak, and mean level.

use super::gain::gain_to_db;
use super::peaks::FramePeaks;

const FULL_SCALE: f64 = 1.0;

// Reaching full scale counts: a mix is clamped to exactly 1.0 on its way to a file.
// Held against the true peak; `fx::limiter`'s `CEILING` sits 1 dB under it, read by the same
// kernel.
const CLIPPING: f64 = FULL_SCALE;

const STEREO: usize = 2;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Loudness {
    pub peak_dbfs: Option<f64>,
    pub true_peak_dbfs: Option<f64>,
    pub mean_dbfs: Option<f64>,
}

impl Loudness {
    pub fn is_silent(&self) -> bool {
        self.peak_dbfs.is_none()
    }

    pub fn is_clipping(&self) -> bool {
        self.true_peak_dbfs
            .is_some_and(|dbfs| dbfs >= ratio_to_dbfs(CLIPPING))
    }
}

#[derive(Debug, Clone)]
pub struct Meter {
    channels: usize,
    peak: f64,
    true_peak: f64,
    sum_of_squares: f64,
    counted: u64,
    sum_of_products: f64,
    sum_of_left: f64,
    sum_of_right: f64,
    peaks: FramePeaks,
}

impl Meter {
    pub fn new(channels: usize) -> Self {
        Self {
            channels: channels.max(1),
            peak: 0.0,
            true_peak: 0.0,
            sum_of_squares: 0.0,
            counted: 0,
            sum_of_products: 0.0,
            sum_of_left: 0.0,
            sum_of_right: 0.0,
            peaks: FramePeaks::new(channels),
        }
    }

    pub fn feed(&mut self, samples: &[f32]) {
        for &sample in samples {
            let magnitude = f64::from(sample).abs();
            self.peak = self.peak.max(magnitude);
            self.sum_of_squares += f64::from(sample) * f64::from(sample);
        }
        self.counted += samples.len() as u64;
        if self.channels == STEREO {
            for frame in samples.chunks_exact(STEREO) {
                let (left, right) = (f64::from(frame[0]), f64::from(frame[1]));
                self.sum_of_products += left * right;
                self.sum_of_left += left * left;
                self.sum_of_right += right * right;
            }
        }
        let true_peak = &mut self.true_peak;
        self.peaks
            .feed(samples, |peak| *true_peak = true_peak.max(peak));
    }

    pub fn finish(&self) -> Loudness {
        if self.counted == 0 || self.peak == 0.0 {
            return Loudness {
                peak_dbfs: None,
                true_peak_dbfs: None,
                mean_dbfs: None,
            };
        }
        let mean_square = self.sum_of_squares / self.counted as f64;
        Loudness {
            peak_dbfs: Some(ratio_to_dbfs(self.peak)),
            // Never under the sample peak: the reconstruction passes through every sample.
            true_peak_dbfs: Some(ratio_to_dbfs(self.true_peak().max(self.peak))),
            mean_dbfs: Some(ratio_to_dbfs(mean_square.sqrt())),
        }
    }

    pub fn correlation(&self) -> Option<f64> {
        if self.channels != STEREO {
            return None;
        }
        let energy = self.sum_of_left * self.sum_of_right;
        if energy <= 0.0 {
            return None;
        }
        // Clamped: two identical channels divide to 1.0 give or take an ulp.
        Some((self.sum_of_products / energy.sqrt()).clamp(-1.0, 1.0))
    }

    fn true_peak(&self) -> f64 {
        let mut peak = self.true_peak;
        self.peaks.tail(|frame| peak = peak.max(frame));
        peak
    }
}

fn ratio_to_dbfs(ratio: f64) -> f64 {
    gain_to_db(ratio)
}
