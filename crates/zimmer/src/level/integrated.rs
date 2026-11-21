//! Integrated loudness, in LUFS: the one number a delivery is held to.

use std::f64::consts::PI;

use super::gain::db_to_gain;

const OFFSET: f64 = -0.691;

const ABSOLUTE_GATE: f64 = -70.0;

const RELATIVE_GATE: f64 = 10.0;

const STEP_SECONDS: f64 = 0.1;

const STEPS_PER_BLOCK: usize = 4;

#[derive(Debug, Clone)]
pub struct Integrated {
    channels: usize,
    filters: Vec<KWeighting>,
    step: usize,
    filled: usize,
    energy: f64,
    steps: Vec<f64>,
}

impl Integrated {
    pub fn new(channels: usize, rate: u32) -> Self {
        let channels = channels.max(1);
        let rate = f64::from(rate.max(1));
        Self {
            channels,
            filters: vec![KWeighting::at(rate); channels],
            step: ((rate * STEP_SECONDS).round() as usize).max(1),
            filled: 0,
            energy: 0.0,
            steps: Vec::new(),
        }
    }

    pub fn feed(&mut self, samples: &[f32]) {
        for frame in samples.chunks_exact(self.channels) {
            for (sample, filter) in frame.iter().zip(&mut self.filters) {
                let weighted = filter.next(f64::from(*sample));
                self.energy += weighted * weighted;
            }
            self.filled += 1;
            if self.filled == self.step {
                self.steps.push(self.energy);
                self.energy = 0.0;
                self.filled = 0;
            }
        }
    }

    pub fn finish(&self) -> Option<f64> {
        let per_block = (self.step * STEPS_PER_BLOCK) as f64;
        let blocks: Vec<f64> = self
            .steps
            .windows(STEPS_PER_BLOCK)
            .map(|window| window.iter().sum::<f64>() / per_block)
            .filter(|&energy| lufs(energy) > ABSOLUTE_GATE)
            .collect();
        let gate = lufs(mean(&blocks)?) - RELATIVE_GATE;
        let loud: Vec<f64> = blocks
            .into_iter()
            .filter(|&energy| lufs(energy) > gate)
            .collect();
        mean(&loud).map(lufs)
    }
}

fn lufs(energy: f64) -> f64 {
    OFFSET + 10.0 * energy.log10()
}

fn mean(values: &[f64]) -> Option<f64> {
    (!values.is_empty()).then(|| values.iter().sum::<f64>() / values.len() as f64)
}

#[derive(Debug, Clone, Copy)]
struct KWeighting {
    shelf: Biquad,
    high_pass: Biquad,
}

impl KWeighting {
    fn at(rate: f64) -> Self {
        // The shelf: +4 dB above about 1.7 kHz.
        let (f0, gain_db, q) = (
            1_681.974_450_955_533,
            3.999_843_853_973_347,
            0.707_175_236_955_419_6,
        );
        let k = (PI * f0 / rate).tan();
        let vh = db_to_gain(gain_db);
        let vb = vh.powf(0.499_666_774_154_541_6);
        let a0 = 1.0 + k / q + k * k;
        let shelf = Biquad::new(
            [
                (vh + vb * k / q + k * k) / a0,
                2.0 * (k * k - vh) / a0,
                (vh - vb * k / q + k * k) / a0,
            ],
            [2.0 * (k * k - 1.0) / a0, (1.0 - k / q + k * k) / a0],
        );
        // The high-pass: the "revised low-frequency B" curve, about 38 Hz.
        let (f0, q) = (38.135_470_876_024_44, 0.500_327_037_323_877_3);
        let k = (PI * f0 / rate).tan();
        let a0 = 1.0 + k / q + k * k;
        let high_pass = Biquad::new(
            [1.0, -2.0, 1.0],
            [2.0 * (k * k - 1.0) / a0, (1.0 - k / q + k * k) / a0],
        );
        Self { shelf, high_pass }
    }

    fn next(&mut self, sample: f64) -> f64 {
        self.high_pass.next(self.shelf.next(sample))
    }
}

#[derive(Debug, Clone, Copy)]
struct Biquad {
    b: [f64; 3],
    a: [f64; 2],
    state: [f64; 2],
}

impl Biquad {
    fn new(b: [f64; 3], a: [f64; 2]) -> Self {
        Self {
            b,
            a,
            state: [0.0; 2],
        }
    }

    fn next(&mut self, x: f64) -> f64 {
        let y = self.b[0] * x + self.state[0];
        self.state[0] = self.b[1] * x - self.a[0] * y + self.state[1];
        self.state[1] = self.b[2] * x - self.a[1] * y;
        y
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn at_48k_the_filters_are_the_standards_table() {
        let k = KWeighting::at(48_000.0);
        let close = |a: f64, b: f64| (a - b).abs() < 1e-9;
        assert!(close(k.shelf.b[0], 1.535_124_859_586_97));
        assert!(close(k.shelf.b[1], -2.691_696_189_406_38));
        assert!(close(k.shelf.b[2], 1.198_392_810_852_85));
        assert!(close(k.shelf.a[0], -1.690_659_293_182_41));
        assert!(close(k.shelf.a[1], 0.732_480_774_215_85));
        assert!(close(k.high_pass.a[0], -1.990_047_454_833_98));
        assert!(close(k.high_pass.a[1], 0.990_072_250_366_21));
    }
}
