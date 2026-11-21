//! The waveform *between* the samples, reconstructed.
//!
//! A sample is not the signal — it is a point on a band-limited curve, and the
//! curve between two samples can exceed both of them. So a buffer whose every
//! sample sits under full scale can still clip the moment anything reconstructs
//! it: a converter on the way to a speaker, a resampler on the way into a mix,
//! an AAC encoder on the way into a delivered video. All three are on paths
//! this workspace already puts audio through.
//!
//! **One copy of the arithmetic, read by two callers, and that is the whole
//! reason it is a module.** [`super::meter`] reports the true peak; the
//! [limiter](crate::fx::limiter) holds a bake under it. When they reconstructed
//! the waveform differently the limiter could satisfy its own guarantee and be
//! reported as clipping by the meter that shipped beside it — which is exactly
//! what happened, and is a class of bug no amount of care in either file would
//! have caught. Sharing the kernel makes the two agree by construction; all
//! that is left to choose is the number each holds against.

/// Oversampling factor.
///
/// Four is what ITU-R BS.1770 asks for at these rates, and it is enough: the
/// error left over is a few hundredths of a decibel, which is well inside the
/// headroom a true-peak ceiling leaves anyway.
const OVERSAMPLE: usize = 4;

/// Half-width of the interpolating kernel, in input samples either side.
///
/// Eight taps total. A windowed sinc rather than linear interpolation, because
/// linear interpolation between two samples never exceeds the larger of them —
/// it would report the sample peak again under a longer name, which is worse
/// than not reporting it.
///
/// Also how far a caller feeding a signal a run at a time has to see in **both**
/// directions: this many frames of the previous run, for the left-hand taps of
/// the frames that follow it, and this many frames held back at the end of the
/// newest run until their right-hand taps arrive. Either half missing
/// reconstructs a segment boundary as an edge in silence, and the ringing off
/// that edge reads as an intersample peak — which is what a meter fed in runs
/// used to report, a decibel hotter than the same signal fed whole.
pub(crate) const TAPS: usize = 4;

/// One channel of an interleaved run, read as the continuous signal it stands
/// for.
///
/// Per channel rather than across the interleaved buffer: interpolating
/// between a left sample and the right sample beside it is interpolating
/// between two different signals, and would invent an excursion at every frame
/// boundary.
pub(crate) struct Channel<'a> {
    samples: &'a [f32],
    channels: usize,
    channel: usize,
    frames: usize,
    /// The kernel, evaluated once per channel rather than once per sample.
    ///
    /// The weights depend only on which tap and which sub-sample position, so
    /// there are `OVERSAMPLE × (2·TAPS + 1)` of them for any signal of any
    /// length.
    /// Computing them inline meant two `sin` calls per tap per step per frame —
    /// fifty-four transcendentals to place one sample — which was affordable
    /// while the meter was the only caller and is not now that the limiter
    /// reconstructs every bake as well.
    weights: [[f64; TAPS * 2 + 1]; OVERSAMPLE],
}

impl<'a> Channel<'a> {
    /// The `channel`th of `channels` woven together in `samples`.
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

    /// A signal that is already one channel on its own.
    pub(crate) fn mono(samples: &'a [f32]) -> Self {
        Self::of(samples, 1, 0)
    }

    /// The largest excursion over the stretch `frame` names: from the sample
    /// itself up to, but not including, the next one.
    ///
    /// Half-open, so walking every frame covers the signal once with nothing
    /// counted twice. The sample itself is the first of the [`OVERSAMPLE`]
    /// points, and it costs nothing to include — at an offset of zero the
    /// kernel is `1` on the centre tap and a zero-crossing on every other, so
    /// the reconstruction *is* the sample. That is what makes this the true
    /// peak of the stretch rather than only of the gaps in it.
    pub(crate) fn peak_from(&self, frame: usize) -> f64 {
        (0..OVERSAMPLE)
            .map(|step| self.between(frame, step).abs())
            .fold(0.0, f64::max)
    }

    /// The value `step` sub-samples of the way past `frame`, with sign.
    fn between(&self, frame: usize, step: usize) -> f64 {
        let mut sum = 0.0;
        for (tap, weight) in self.weights[step].iter().enumerate() {
            let at = frame as isize + tap as isize - TAPS as isize;
            sum += self.at(at) * weight;
        }
        sum
    }

    /// One sample of this channel. Outside the run it reads as zero, which is
    /// what the signal is before it starts and after it ends.
    fn at(&self, frame: isize) -> f64 {
        if frame < 0 || frame as usize >= self.frames {
            return 0.0;
        }
        f64::from(self.samples[frame as usize * self.channels + self.channel])
    }
}

/// The Lanczos kernel, zero outside [`TAPS`].
fn lanczos(x: f64) -> f64 {
    if x.abs() >= TAPS as f64 {
        return 0.0;
    }
    sinc(x) * sinc(x / TAPS as f64)
}

/// Normalised sinc, `1` at zero.
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

    /// A lone spike in silence, `at` frames from the start.
    fn spike(len: usize, at: usize) -> Vec<f32> {
        let mut buf = vec![0.0; len];
        buf[at] = 1.0;
        buf
    }

    /// The case the whole module exists for, in three samples: a signal whose
    /// samples are all under full scale and whose waveform is not.
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

    /// A signal that is not moving reconstructs to itself, **sign and all** —
    /// to within a quarter of a percent, which is where the truncated kernel
    /// lands and is the error every reading here carries.
    ///
    /// Asserted on the signed value rather than on the magnitude, because both
    /// callers take an absolute value and a reconstruction that came back
    /// negated would satisfy every one of their assertions. The error errs
    /// *upward*, which is the direction both want: a meter that over-reports
    /// full scale by two hundredths of a decibel says "clipping" a shade
    /// early, and a limiter reading the same way ducks a shade harder than it
    /// had to.
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

    /// A frame's stretch begins *at* the frame, and the kernel is built so
    /// that costs nothing: at an offset of zero every tap but the centre one
    /// sits on a zero of the sinc, so the reading is the sample itself.
    ///
    /// The first and last frames are asserted on rather than a comfortable
    /// middle. A reconstruction filter goes wrong at its edges, and a bake's
    /// loudest transient is very often its opening sample.
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

    /// A reading is about the stretch it names and no other, and it falls away
    /// with distance. A reading that found a distant spike *loudly* would be a
    /// kernel centred somewhere other than the frame it was asked about, which
    /// is exactly what a mis-scaled sub-sample offset builds.
    #[test]
    fn a_reading_covers_the_stretch_it_names_and_no_other() {
        let buf = spike(64, 32);
        let channel = Channel::mono(&buf);
        assert!(channel.peak_from(32) >= 1.0, "the stretch holding it");
        assert!(
            channel.peak_from(31) > 0.1,
            "and its neighbour hears the near side of it"
        );
        // Four frames out, only the outermost tap still touches it, faintly.
        let edge = channel.peak_from(28);
        assert!(edge < 0.1, "the spike carried four frames at {edge}");
        // Five and further, the kernel does not reach at all.
        for far in [18, 22, 27, 37, 41, 45] {
            let read = channel.peak_from(far);
            assert!(read < 1e-9, "frame {far} reached frame 32: {read}");
        }
    }

    /// Reading one channel of an interleaved pair must never mix the two —
    /// that is what would invent an excursion at every frame boundary. Here
    /// each channel on its own is a constant while the interleaved buffer
    /// alternates, so a reading that strayed across would be unmissable.
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
