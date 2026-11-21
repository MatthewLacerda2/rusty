//! Integrated loudness, in LUFS: the one number a delivery is held to.
//!
//! [`super::Meter`]'s mean is an RMS, and it answers *is this stretch as loud
//! as that one* well enough. It is not what a feed normalises playback to.
//! Every platform that publishes a loudness figure publishes it in **LUFS**
//! (ITU-R BS.1770-4, *Algorithms to measure audio programme loudness and
//! true-peak audio level*), which differs from an RMS in two ways that both
//! matter to a soundtrack:
//!
//! - **It weights the spectrum the way hearing does** (the *K-weighting*): a
//!   high shelf that counts the presence range a little more, and a high-pass
//!   that counts rumble hardly at all. A bass-heavy music bed and a bright
//!   voice at the same RMS are not equally loud to a listener, and LUFS says so.
//! - **It ignores silence and near-silence** (the *gating*): the mean is taken
//!   over 400 ms blocks, overlapping by three quarters, and the blocks under
//!   −70 LUFS, then the blocks more than 10 LU under the mean of the rest, are
//!   left out. Five seconds of quiet before an ad's first line does not make
//!   the ad read quieter.
//!
//! The arithmetic is the standard's, line for line, and the coefficients for
//! the two filters are derived for the rate the signal is at rather than
//! copied from its 48 kHz table, so a delivery at 44.1 kHz is weighted by the
//! same curve. EBU Tech 3341's reference signals are what the tests hold it to.
//!
//! **Every channel counts once.** The standard weights surround channels
//! by 1.41; nothing this workspace makes has any, so a channel is a channel.

use std::f64::consts::PI;

use super::gain::db_to_gain;

/// The offset in the standard's definition, which makes a 997 Hz tone read
/// its own RMS level in LUFS once the K-weighting is applied.
const OFFSET: f64 = -0.691;

/// The absolute gate: a block quieter than this is silence, in LUFS.
const ABSOLUTE_GATE: f64 = -70.0;

/// The relative gate: how far under the absolute-gated mean a block may sit
/// and still count, in LU.
const RELATIVE_GATE: f64 = 10.0;

/// One step of the block grid, in seconds. A block is four of these, so
/// consecutive blocks overlap by three quarters as the standard asks.
const STEP_SECONDS: f64 = 0.1;

/// How many steps make a gating block of 400 ms.
const STEPS_PER_BLOCK: usize = 4;

/// Measures the integrated loudness of an interleaved signal fed a run at a
/// time.
///
/// What it keeps is the energy of each 100 ms step, not the samples: an hour
/// is 36,000 numbers.
#[derive(Debug, Clone)]
pub struct Integrated {
    channels: usize,
    /// One weighting filter per channel, each with its own memory.
    filters: Vec<KWeighting>,
    /// Samples per channel in one step of the block grid.
    step: usize,
    /// How far into the current step the signal is, in frames.
    filled: usize,
    /// The summed, weighted energy of the current step so far.
    energy: f64,
    /// The energy of every finished step, in order.
    steps: Vec<f64>,
}

impl Integrated {
    /// A meter for `channels` interleaved channels at `rate` samples a second.
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

    /// Takes another run of interleaved samples.
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

    /// The integrated loudness in LUFS, or `None` when there is none to speak
    /// of: a signal under 400 ms, which fills no gating block, or one whose
    /// every block is under the absolute gate — silence, as far as a listener
    /// is concerned.
    ///
    /// A part-filled final block is left out, as the standard's block grid
    /// leaves it.
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

/// The loudness a block of mean weighted energy `energy` reads as.
fn lufs(energy: f64) -> f64 {
    OFFSET + 10.0 * energy.log10()
}

/// The mean of `values`, or `None` for none.
fn mean(values: &[f64]) -> Option<f64> {
    (!values.is_empty()).then(|| values.iter().sum::<f64>() / values.len() as f64)
}

/// The K-weighting of one channel: the standard's high shelf, then its
/// high-pass, as two biquads in series.
#[derive(Debug, Clone, Copy)]
struct KWeighting {
    shelf: Biquad,
    high_pass: Biquad,
}

impl KWeighting {
    /// The two filters, designed for `rate`.
    ///
    /// The analogue prototypes behind the standard's 48 kHz coefficients,
    /// brought to `rate` by the bilinear transform. At 48 kHz they reproduce
    /// the published table to its last printed digit.
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

    /// The next weighted sample.
    fn next(&mut self, sample: f64) -> f64 {
        self.high_pass.next(self.shelf.next(sample))
    }
}

/// A second-order section, direct form II transposed.
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

    /// The standard's own 48 kHz table, which the design above has to land on.
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
