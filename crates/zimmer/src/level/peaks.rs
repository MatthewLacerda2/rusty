//! The true peak of each sample-frame, for a signal that arrives a run at a time.

use super::intersample::{Channel, TAPS};

#[derive(Debug, Clone)]
pub struct FramePeaks {
    channels: usize,
    recent: Vec<f32>,
    settled: usize,
}

impl FramePeaks {
    pub fn new(channels: usize) -> Self {
        Self {
            channels: channels.max(1),
            recent: Vec::new(),
            settled: 0,
        }
    }

    pub fn feed(&mut self, samples: &[f32], mut each: impl FnMut(f64)) {
        let mut joined = std::mem::take(&mut self.recent);
        joined.extend_from_slice(samples);
        let frames = joined.len() / self.channels;
        // `max` rather than a bare subtraction: a run shorter than the kernel adds no measurable
        // frames at all, and must not un-measure any.
        let ready = frames.saturating_sub(TAPS).max(self.settled);
        self.measure(&joined, self.settled..ready, &mut each);
        let drop = ready.saturating_sub(TAPS);
        self.settled = ready - drop;
        joined.drain(..drop * self.channels);
        self.recent = joined;
    }

    pub fn tail(&self, mut each: impl FnMut(f64)) {
        let frames = self.recent.len() / self.channels;
        self.measure(&self.recent, self.settled..frames, &mut each);
    }

    fn measure(&self, samples: &[f32], range: std::ops::Range<usize>, each: &mut impl FnMut(f64)) {
        let channels: Vec<Channel<'_>> = (0..self.channels)
            .map(|index| Channel::of(samples, self.channels, index))
            .collect();
        for frame in range {
            each(
                channels
                    .iter()
                    .map(|channel| channel.peak_from(frame))
                    .fold(0.0, f64::max),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_reader_keeps_a_window_on_the_seam_and_never_the_signal() {
        for channels in [1, 2] {
            let mut peaks = FramePeaks::new(channels);
            for _ in 0..20 {
                peaks.feed(&vec![0.5; 4_096], |_| {});
                assert!(
                    peaks.recent.len() <= 2 * TAPS * channels,
                    "{channels} channels held {} samples",
                    peaks.recent.len()
                );
            }
        }
    }
}
