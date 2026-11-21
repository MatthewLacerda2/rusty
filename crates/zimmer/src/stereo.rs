//! Two channels, kept in two buffers.
//!
//! The shape every stage of the signal path works in once a source has
//! produced something. Left and right are always the same length, and every
//! operation here preserves that — a signal whose channels disagree about how
//! long it is has no meaning, and there is nowhere in this crate one could
//! legitimately arise.
//!
//! **Why split rather than interleaved** is argued in [the crate doc](crate);
//! the short of it is that nearly every function in this crate takes
//! `&mut [f32]`, and this shape lets them go on doing that.
//!
//! [`Stereo::each`] is the workhorse: it hands one channel at a time to a
//! function that has never heard of the other, which is what a filter, an
//! envelope, a waveshaper and a delay line all are. The stages that genuinely
//! need both sides at once — the linked limiter, the stereo reverb's mono
//! send, the pan that puts a mono part somewhere — reach for `l` and `r`
//! directly, and there are few enough of them to name.
//!
//! **Interleaving happens exactly twice**, at the two edges where a buffer
//! stops being a signal and becomes something else: the WAV encoder, and the
//! meter. Both want the whole thing at once, so neither pays for the strided
//! access an interleaved signal path would have imposed on everything between
//! them.

use std::f32::consts::{FRAC_PI_4, SQRT_2};

/// How many channels this crate renders in. Two, and see [the crate
/// doc](crate) for why it is a fixed number rather than a setting.
pub(crate) const CHANNELS: usize = 2;

/// A stereo signal: two channels of equal length.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Stereo {
    /// The left channel.
    pub(crate) l: Vec<f32>,
    /// The right channel, always [`Stereo::l`]'s length.
    pub(crate) r: Vec<f32>,
}

impl Stereo {
    /// `frames` sample-frames of silence.
    pub(crate) fn silence(frames: usize) -> Self {
        Self {
            l: vec![0.0; frames],
            r: vec![0.0; frames],
        }
    }

    /// One signal placed in both channels — a mono part, dead centre.
    ///
    /// The two channels are then *identical*, not merely similar, which is
    /// what makes a song that pans nothing render the samples it always did
    /// into both sides.
    pub(crate) fn centred(mono: Vec<f32>) -> Self {
        Self {
            l: mono.clone(),
            r: mono,
        }
    }

    /// How many sample-frames long it is.
    pub(crate) fn frames(&self) -> usize {
        self.l.len()
    }

    /// The two samples at frame `i`, or silence past the end.
    ///
    /// For a stage that reads one signal while writing another of the same
    /// length — a room fed from a send — where running out is not an error
    /// but the end of what was sent.
    pub(crate) fn frame(&self, i: usize) -> (f32, f32) {
        let side = |channel: &[f32]| channel.get(i).copied().unwrap_or(0.0);
        (side(&self.l), side(&self.r))
    }

    /// True when there is nothing in it.
    pub(crate) fn is_empty(&self) -> bool {
        self.l.is_empty()
    }

    /// Cuts or pads both channels to `frames`, padding with silence.
    pub(crate) fn resize(&mut self, frames: usize) {
        self.l.resize(frames, 0.0);
        self.r.resize(frames, 0.0);
    }

    /// Grows both channels to at least `frames`, leaving a longer signal alone.
    ///
    /// Written as a `max` rather than as a guarded resize because the guard
    /// has no edge to get wrong: resizing to the length it already is does
    /// nothing, so `>` and `>=` would behave identically and no test could
    /// ever tell them apart.
    pub(crate) fn grow_to(&mut self, frames: usize) {
        self.resize(frames.max(self.frames()));
    }

    /// Adds `other` to this signal sample for sample, from the first frame,
    /// growing to the longer of the two.
    pub(crate) fn add(&mut self, other: &Stereo) {
        self.grow_to(other.frames());
        for (mine, theirs) in [(&mut self.l, &other.l), (&mut self.r, &other.r)] {
            for (sample, added) in mine.iter_mut().zip(theirs) {
                *sample += added;
            }
        }
    }

    /// Keeps only the frames `from..to`, clamped to what is there.
    ///
    /// The one operation that moves a signal's start: everything else here
    /// grows or trims an end. What it exists for is an excerpt — a whole
    /// render, cut down to the stretch that was asked about — so it drops the
    /// front rather than fading it. A window's edge is not an edit, it is a
    /// place to read numbers off; nothing plays across it.
    pub(crate) fn cut(&mut self, from: usize, to: usize) {
        let frames = self.frames();
        let from = from.min(frames);
        let to = to.clamp(from, frames);
        for channel in [&mut self.l, &mut self.r] {
            channel.truncate(to);
            channel.drain(..from);
        }
    }

    /// Runs `apply` over each channel in turn.
    ///
    /// The way every mono stage reaches a stereo signal: a filter, an
    /// envelope, a waveshaper and a delay line each hold state that belongs to
    /// one channel, and none of them has any business seeing the other.
    pub(crate) fn each(&mut self, mut apply: impl FnMut(&mut [f32])) {
        apply(&mut self.l);
        apply(&mut self.r);
    }

    /// The two channels woven into one buffer, left sample first.
    ///
    /// The interchange form: what a WAV file holds and what [`crate::level`]
    /// measures. Everything between those two edges works in the split form
    /// instead.
    pub(crate) fn interleaved(&self) -> Vec<f32> {
        let mut out = Vec::with_capacity(self.frames() * CHANNELS);
        for (l, r) in self.l.iter().zip(&self.r) {
            out.push(*l);
            out.push(*r);
        }
        out
    }
}

/// The left and right gains a `pan` of `-1..=1` implies.
///
/// **Constant power**, not linear: the two gains sweep a quarter circle rather
/// than a straight line, so the sum of their squares is the same wherever the
/// part sits. A linear law holds the *sum* constant instead, which drops a
/// hard-panned part 3 dB the moment it leaves the middle — an instrument that
/// gets quieter as it moves is a pan control nobody can use.
///
/// The circle is scaled so that **centre is unity** rather than −3 dB. Both
/// conventions are constant-power, and the choice between them is a choice of
/// what stays put: this one keeps a centred part at exactly the level it was
/// written at and lets a hard-panned one arrive 3 dB up on the side it went
/// to. The other would have quietened every existing recipe by 3 dB for
/// changing nothing, and a bake that comes back under the version it replaced
/// is the exact defect [`crate::level`] exists to catch. What it costs is that
/// moving a part that was already near full scale hands the master limiter 3 dB
/// to deal with — which is what the master limiter is for, and a quieter
/// version of every recipe is not.
///
/// Dead centre is returned as a literal `(1.0, 1.0)` rather than computed:
/// `√2·cos(π/4)` is `0.99999994` in `f32`, and a track that never mentioned
/// `pan` must render the samples it always did. That is the same promise, and
/// the same reason, as a track with no fx chain not being bussed at all — see
/// [`crate::song`]'s mixer.
pub(crate) fn pan_gains(pan: f32) -> (f32, f32) {
    if !pan.is_finite() || pan == 0.0 {
        return (1.0, 1.0);
    }
    let angle = (pan.clamp(-1.0, 1.0) + 1.0) * FRAC_PI_4;
    (
        (SQRT_2 * angle.cos()).max(0.0),
        (SQRT_2 * angle.sin()).max(0.0),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ramp(n: usize) -> Vec<f32> {
        (0..n).map(|i| i as f32).collect()
    }

    #[test]
    fn a_centred_signal_is_the_same_buffer_twice() {
        let stereo = Stereo::centred(ramp(4));
        assert_eq!(stereo.l, stereo.r);
        assert_eq!(stereo.frames(), 4);
        assert_eq!(
            stereo.interleaved(),
            vec![0.0, 0.0, 1.0, 1.0, 2.0, 2.0, 3.0, 3.0]
        );
    }

    #[test]
    fn silence_is_empty_only_when_it_has_no_frames() {
        assert!(Stereo::silence(0).is_empty());
        assert!(!Stereo::silence(1).is_empty());
        assert_eq!(Stereo::default(), Stereo::silence(0));
    }

    #[test]
    fn resizing_moves_both_channels_together() {
        let mut stereo = Stereo::centred(ramp(4));
        stereo.resize(2);
        assert_eq!(stereo.l, vec![0.0, 1.0]);
        assert_eq!(stereo.r, vec![0.0, 1.0]);
        stereo.grow_to(4);
        assert_eq!(stereo.l, vec![0.0, 1.0, 0.0, 0.0], "padded with silence");
        stereo.grow_to(1);
        assert_eq!(stereo.frames(), 4, "growing never shortens");
    }

    #[test]
    fn each_channel_is_visited_once_and_on_its_own() {
        let mut stereo = Stereo {
            l: vec![1.0],
            r: vec![2.0],
        };
        let mut seen = Vec::new();
        stereo.each(|channel| {
            seen.push(channel[0]);
            channel[0] *= 10.0;
        });
        assert_eq!(seen, vec![1.0, 2.0], "left first, then right");
        assert_eq!(stereo.l, vec![10.0]);
        assert_eq!(stereo.r, vec![20.0]);
    }

    /// The promise a default has to keep: a track that never says `pan` is
    /// scaled by exactly one, on both sides, so its samples are the ones it
    /// always had.
    #[test]
    fn dead_centre_is_exactly_unity() {
        assert_eq!(pan_gains(0.0), (1.0, 1.0));
        assert_eq!(pan_gains(-0.0), (1.0, 1.0));
        for absurd in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            assert_eq!(pan_gains(absurd), (1.0, 1.0), "{absurd} is not a position");
        }
    }

    /// Hard over is one side and silence on the other, at `√2` — the +3 dB
    /// that keeps the power constant as it leaves the middle.
    #[test]
    fn hard_over_is_all_of_one_side_and_none_of_the_other() {
        let (l, r) = pan_gains(-1.0);
        assert!((l - SQRT_2).abs() < 1e-6, "left is {l}");
        assert_eq!(r, 0.0, "and nothing bleeds to the right");
        let (l, r) = pan_gains(1.0);
        assert_eq!(l, 0.0);
        assert!((r - SQRT_2).abs() < 1e-6, "right is {r}");
        // Three and minus three rather than some larger absurdity: the law is
        // a quarter circle, so a big enough number comes back around onto a
        // legal-looking pair by accident. These two land on the far side of
        // it, where an unclamped law returns silence.
        assert_eq!(
            pan_gains(-3.0),
            pan_gains(-1.0),
            "past hard over is hard over"
        );
        assert_eq!(pan_gains(3.0), pan_gains(1.0));
    }

    /// The property the law is named for, and the one a linear law does not
    /// have: the power is the same wherever the part is put.
    #[test]
    fn the_power_is_constant_across_the_whole_sweep() {
        for step in -10..=10 {
            let pan = step as f32 / 10.0;
            let (l, r) = pan_gains(pan);
            let power = l * l + r * r;
            assert!(
                (power - 2.0).abs() < 1e-5,
                "pan {pan} carries power {power}"
            );
        }
    }

    /// A pan is a position, so it moves monotonically and symmetrically.
    #[test]
    fn panning_right_moves_level_from_left_to_right() {
        let (half_l, half_r) = pan_gains(0.5);
        assert!(half_r > 1.0 && half_l < 1.0, "{half_l} / {half_r}");
        let (mirror_l, mirror_r) = pan_gains(-0.5);
        assert!((mirror_l - half_r).abs() < 1e-6, "the sweep is symmetric");
        assert!((mirror_r - half_l).abs() < 1e-6);
    }
}
