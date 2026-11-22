//! The waveform *between* the samples, reconstructed.
//! `fx::limiter` and the meter's true peak share this kernel, so a bake's limit and its reported
//! peak agree.

/// ITU-R BS.1770's true-peak oversampling.
const OVERSAMPLE: usize = 4;

/// Lanczos half-width, in frames: a caller fed in runs keeps this many either side of a seam.
pub(crate) const TAPS: usize = 4;

pub(crate) struct Channel<'a> {
    samples: &'a [f32],
    channels: usize,
    channel: usize,
    frames: usize,
    weights: [[f64; TAPS * 2 + 1]; OVERSAMPLE],
}

impl<'a> Channel<'a> {
    pub(crate) fn of(samples: &'a [f32], channels: usize, channel: usize) -> Self {
        let channels = channels.max(1);
        let mut weights = [[0.0; TAPS * 2 + 1]; OVERSAMPLE];
        for (step, row) in weights.iter_mut().enumerate() {
            let offset = step as f64 / OVERSAMPLE as f64;
            for (tap, weight) in row.iter_mut().enumerate() {
                *weight = lanczos(tap as f64 - TAPS as f64 - offset);
            }
        }
        Self {
            samples,
            channels,
            channel,
            frames: samples.len() / channels,
            weights,
        }
    }

    pub(crate) fn mono(samples: &'a [f32]) -> Self {
        Self::of(samples, 1, 0)
    }

    /// Half-open, from `frame` up to the next, so walking every frame covers each stretch once.
    pub(crate) fn peak_from(&self, frame: usize) -> f64 {
        (0..OVERSAMPLE)
            .map(|step| self.between(frame, step).abs())
            .fold(0.0, f64::max)
    }

    fn between(&self, frame: usize, step: usize) -> f64 {
        let mut sum = 0.0;
        for (tap, weight) in self.weights[step].iter().enumerate() {
            let at = frame as isize + tap as isize - TAPS as isize;
            sum += self.at(at) * weight;
        }
        sum
    }

    fn at(&self, frame: isize) -> f64 {
        if frame < 0 || frame as usize >= self.frames {
            return 0.0;
        }
        f64::from(self.samples[frame as usize * self.channels + self.channel])
    }
}

// A windowed sinc: linear interpolation never exceeds either sample, so it would re-report the
// sample peak.
fn lanczos(x: f64) -> f64 {
    if x.abs() >= TAPS as f64 {
        return 0.0;
    }
    sinc(x) * sinc(x / TAPS as f64)
}

fn sinc(x: f64) -> f64 {
    if x == 0.0 {
        return 1.0;
    }
    let x = std::f64::consts::PI * x;
    x.sin() / x
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spike(len: usize, at: usize) -> Vec<f32> {
        let mut buf = vec![0.0; len];
        buf[at] = 1.0;
        buf
    }

    #[test]
    fn the_curve_between_two_samples_can_exceed_both() {
        // A quarter-rate tone read half a sample late: +a, +a, -a, -a.
        let a = 0.7;
        let tone: Vec<f32> = (0..64)
            .map(|i| if (i / 2) % 2 == 0 { a } else { -a })
            .collect();
        let channel = Channel::mono(&tone);
        let peak = (16..48).map(|f| channel.peak_from(f)).fold(0.0, f64::max);
        assert!(peak > f64::from(a) * 1.2, "read only {peak}");
    }

    #[test]
    fn a_constant_reconstructs_to_itself_sign_and_all() {
        let flat = vec![0.5; 64];
        let channel = Channel::mono(&flat);
        for frame in 8..56 {
            for step in 0..OVERSAMPLE {
                let read = channel.between(frame, step);
                assert!(read >= 0.5, "frame {frame} step {step} read {read}");
                assert!(read < 0.5 * 1.005, "frame {frame} step {step} read {read}");
            }
        }
    }

    #[test]
    fn the_stretch_begins_at_the_sample_itself() {
        for at in [0, 1, 3, 30, 63] {
            let buf = spike(64, at);
            let channel = Channel::mono(&buf);
            let read = channel.between(at, 0);
            assert!((read - 1.0).abs() < 1e-9, "frame {at} read as {read}");
            assert!(channel.peak_from(at) >= 1.0, "and so does its stretch");
        }
    }

    #[test]
    fn a_reading_covers_the_stretch_it_names_and_no_other() {
        let buf = spike(64, 32);
        let channel = Channel::mono(&buf);
        assert!(channel.peak_from(32) >= 1.0, "the stretch holding it");
        assert!(
            channel.peak_from(31) > 0.1,
            "and its neighbour hears the near side of it"
        );
        let edge = channel.peak_from(28);
        assert!(edge < 0.1, "the spike carried four frames at {edge}");
        for far in [18, 22, 27, 37, 41, 45] {
            let read = channel.peak_from(far);
            assert!(read < 1e-9, "frame {far} reached frame 32: {read}");
        }
    }

    #[test]
    fn a_channel_reads_only_its_own_samples() {
        let interleaved: Vec<f32> = (0..64)
            .map(|i| if i % 2 == 0 { 1.0 } else { -1.0 })
            .collect();
        let left = Channel::of(&interleaved, 2, 0);
        let right = Channel::of(&interleaved, 2, 1);
        assert_eq!(left.frames, 32);
        assert_eq!(right.frames, 32);
        assert!(left.between(16, 0) > 0.0, "the left side is the high one");
        assert!(right.between(16, 0) < 0.0, "and the right is not");
        assert!((left.peak_from(16) - 1.0).abs() < 0.005);
        assert!((right.peak_from(16) - 1.0).abs() < 0.005);
    }

    #[test]
    fn an_empty_run_has_no_frames_and_reads_as_silence() {
        let channel = Channel::mono(&[]);
        assert_eq!(channel.frames, 0);
        assert_eq!(channel.peak_from(0), 0.0);
        assert_eq!(channel.at(-1), 0.0);
    }
}
