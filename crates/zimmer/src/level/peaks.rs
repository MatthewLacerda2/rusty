//! The true peak of each sample-frame, for a signal that arrives a run at a
//! time.
//!
//! [`super::Meter`] wants the largest of these and a limiter wants every one
//! of them, so they are worked out here once, from [`super::intersample`]'s
//! reconstruction — the same one the meter reports with. That is what lets a
//! limiter outside this crate (`scorsese-render` holds a delivered mix under a
//! ceiling, #968) promise exactly what the meter beside it will measure,
//! without the reconstruction itself being published: a caller gets the
//! numbers, never the kernel.
//!
//! **A frame is only measurable once both its neighbourhoods exist.** The
//! kernel reaches [`TAPS`] frames either side of the frame it reconstructs,
//! and anything outside the buffer reads as zero — so a frame measured while
//! it still sits at the end of the newest run is measured against a silence
//! that is about to be replaced by real samples. That fabricated edge rings,
//! the ringing is an excursion, and a running maximum keeps it forever: a
//! signal fed in 4 KB runs used to read a decibel hotter than the same signal
//! fed whole.
//!
//! So each run measures the frames from `settled` up to `TAPS` short of the
//! end, keeps the last `TAPS` measured frames as the next run's left-hand
//! context, and holds the rest back. The one place the zeros are real is the
//! end of the signal, which is [`FramePeaks::tail`]'s business.

use super::intersample::{Channel, TAPS};

/// Reads the true peak of every sample-frame of an interleaved signal, louder
/// channel first, in order, whatever size of run it is fed in.
#[derive(Debug, Clone)]
pub struct FramePeaks {
    channels: usize,
    /// The end of the signal so far: the frames not yet measured, preceded by
    /// the [`TAPS`] already-measured frames that are their left-hand context.
    ///
    /// Never more than twice that many frames, whatever a caller feeds — this
    /// is a window on the seam and not a copy of the signal.
    recent: Vec<f32>,
    /// How many leading frames of [`FramePeaks::recent`] have already been
    /// measured. The rest are waiting for their right-hand neighbours.
    settled: usize,
}

impl FramePeaks {
    /// A reader for a signal of `channels` interleaved channels.
    pub fn new(channels: usize) -> Self {
        Self {
            channels: channels.max(1),
            recent: Vec::new(),
            settled: 0,
        }
    }

    /// Takes another run of interleaved samples and hands `each` the peak, as
    /// a linear magnitude, of every frame that has become measurable — in
    /// order, each frame exactly once over the life of the reader.
    ///
    /// A frame's peak is the larger of its channels', which is what a linked
    /// limiter needs and what a meter's maximum comes to anyway.
    pub fn feed(&mut self, samples: &[f32], mut each: impl FnMut(f64)) {
        let mut joined = std::mem::take(&mut self.recent);
        joined.extend_from_slice(samples);
        let frames = joined.len() / self.channels;
        // `max` rather than a bare subtraction: a run shorter than the kernel
        // adds no measurable frames at all, and must not un-measure any.
        let ready = frames.saturating_sub(TAPS).max(self.settled);
        self.measure(&joined, self.settled..ready, &mut each);
        let drop = ready.saturating_sub(TAPS);
        self.settled = ready - drop;
        joined.drain(..drop * self.channels);
        self.recent = joined;
    }

    /// The peaks of the frames still held back, read as if the signal ended
    /// where the samples fed so far do.
    ///
    /// Read rather than consumed, so a meter can ask twice — or ask and then
    /// feed more — and get the answer for the signal as it stands each time.
    /// A caller that has stopped feeding calls this once to finish.
    pub fn tail(&self, mut each: impl FnMut(f64)) {
        let frames = self.recent.len() / self.channels;
        self.measure(&self.recent, self.settled..frames, &mut each);
    }

    /// Hands `each` the louder channel's peak for every frame in `range` of
    /// `samples`.
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

    /// **A reader never holds the signal**, which is the reason it is fed a
    /// run at a time at all. What it keeps between runs is a window on the
    /// seam, so twice the kernel's reach is the ceiling, whatever it is fed
    /// and however often.
    ///
    /// Asserted from inside the module because the buffer is private and there
    /// is no reason for it not to be. The invariant is real all the same: a
    /// reader that quietly retained everything would report exactly the same
    /// numbers and would make a long render run out of memory.
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
