//! Two channels, kept in two buffers.

use std::f32::consts::{FRAC_PI_4, SQRT_2};

pub(crate) const CHANNELS: usize = 2;

/// Two channels always of equal length; every method here keeps them so.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Stereo {
    pub(crate) l: Vec<f32>,
    pub(crate) r: Vec<f32>,
}

impl Stereo {
    pub(crate) fn silence(frames: usize) -> Self {
        Self {
            l: vec![0.0; frames],
            r: vec![0.0; frames],
        }
    }

    pub(crate) fn centred(mono: Vec<f32>) -> Self {
        Self {
            l: mono.clone(),
            r: mono,
        }
    }

    pub(crate) fn frames(&self) -> usize {
        self.l.len()
    }

    pub(crate) fn frame(&self, i: usize) -> (f32, f32) {
        let side = |channel: &[f32]| channel.get(i).copied().unwrap_or(0.0);
        (side(&self.l), side(&self.r))
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.l.is_empty()
    }

    pub(crate) fn resize(&mut self, frames: usize) {
        self.l.resize(frames, 0.0);
        self.r.resize(frames, 0.0);
    }

    pub(crate) fn grow_to(&mut self, frames: usize) {
        self.resize(frames.max(self.frames()));
    }

    pub(crate) fn add(&mut self, other: &Stereo) {
        self.grow_to(other.frames());
        for (mine, theirs) in [(&mut self.l, &other.l), (&mut self.r, &other.r)] {
            for (sample, added) in mine.iter_mut().zip(theirs) {
                *sample += added;
            }
        }
    }

    pub(crate) fn cut(&mut self, from: usize, to: usize) {
        let frames = self.frames();
        let from = from.min(frames);
        let to = to.clamp(from, frames);
        for channel in [&mut self.l, &mut self.r] {
            channel.truncate(to);
            channel.drain(..from);
        }
    }

    pub(crate) fn each(&mut self, mut apply: impl FnMut(&mut [f32])) {
        apply(&mut self.l);
        apply(&mut self.r);
    }

    pub(crate) fn interleaved(&self) -> Vec<f32> {
        let mut out = Vec::with_capacity(self.frames() * CHANNELS);
        for (l, r) in self.l.iter().zip(&self.r) {
            out.push(*l);
            out.push(*r);
        }
        out
    }
}

/// Constant power, scaled so centre is unity: hard over is √2, +3 dB on that side.
pub(crate) fn pan_gains(pan: f32) -> (f32, f32) {
    // A literal: `√2·cos(π/4)` is 0.99999994 in `f32`, and an unpanned track must keep its samples.
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

    #[test]
    fn dead_centre_is_exactly_unity() {
        assert_eq!(pan_gains(0.0), (1.0, 1.0));
        assert_eq!(pan_gains(-0.0), (1.0, 1.0));
        for absurd in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            assert_eq!(pan_gains(absurd), (1.0, 1.0), "{absurd} is not a position");
        }
    }

    #[test]
    fn hard_over_is_all_of_one_side_and_none_of_the_other() {
        let (l, r) = pan_gains(-1.0);
        assert!((l - SQRT_2).abs() < 1e-6, "left is {l}");
        assert_eq!(r, 0.0, "and nothing bleeds to the right");
        let (l, r) = pan_gains(1.0);
        assert_eq!(l, 0.0);
        assert!((r - SQRT_2).abs() < 1e-6, "right is {r}");
        // ±3, not a larger absurdity: the law is a circle, and these land where an unclamped one is
        // silent.
        assert_eq!(
            pan_gains(-3.0),
            pan_gains(-1.0),
            "past hard over is hard over"
        );
        assert_eq!(pan_gains(3.0), pan_gains(1.0));
    }

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

    #[test]
    fn panning_right_moves_level_from_left_to_right() {
        let (half_l, half_r) = pan_gains(0.5);
        assert!(half_r > 1.0 && half_l < 1.0, "{half_l} / {half_r}");
        let (mirror_l, mirror_r) = pan_gains(-0.5);
        assert!((mirror_l - half_r).abs() < 1e-6, "the sweep is symmetric");
        assert!((mirror_r - half_l).abs() < 1e-6);
    }
}
