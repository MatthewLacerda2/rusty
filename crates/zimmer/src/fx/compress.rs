//! the compressor a recipe chooses — and the one effect here that can be
//! listening to something other than the signal it changes.
//!
//! ## Not the limiter at a different setting
//!
//! [`super::limiter`] is a **safety device**. It is not listed among the
//! effects at all, every bake goes through it whether the recipe asked or not,
//! and a signal that never approaches the ceiling comes back untouched — its
//! whole job is that a file cannot clip. A compressor is a **musical device**:
//! it is chosen, it is meant to be audible, and it decides how a part sits.
//! The two share the shape of their implementation and nothing else.
//!
//! What it buys is **crest factor** — the distance between a signal's peaks
//! and its body. A mix with nothing compressing anywhere has peaks towering
//! over its average: it measures loud, sounds quiet, and reads as several
//! parts summed rather than as one performance. Pulling the peaks down and
//! handing the difference back as `makeup` is what closes that gap, and a
//! gentle one over the sum is most of what the word *glue* means.
//!
//! ## Offline, so the attack is free
//!
//! The trick [`super::limiter`]'s doc already argues for: this crate renders a
//! whole buffer rather than streaming it, so the gain every sample needs can
//! be worked out first and *then* smoothed: already down before a peak
//! arrives, and recovering over the release in decibels, so a light duck and a
//! deep one let go over the same time. [`super::gain`] has the passes, shared
//! with the limiter, and why the detector holds a peak rather than chasing the
//! waveform.
//!
//! That is a transparent attack rather than a click followed by an overshoot,
//! and it is the one place this behaves unlike a hardware compressor: the duck
//! is *centred on* the transient instead of chasing it, so a slow attack here
//! does not let the transient through the way a streaming compressor's does.
//! `mix` is how a recipe keeps one — a hard-compressed copy under an untouched
//! one is parallel, or "New York", compression, and it is the safest way to
//! add density without flattening what made the part interesting.
//!
//! ## The detector is linked, and it may be listening elsewhere
//!
//! One gain track, taken from whichever channel is louder at each frame and
//! applied to both, for exactly the reason the limiter gives: an unlinked
//! compressor is a fader that moves one side and not the other every time it
//! acts, and a centred part drifts away from whichever side happened to be
//! loud.
//!
//! *Where* the level is read is the other half. Normally it is the signal
//! being changed. With a sidechain it is another track's part, and what comes
//! out is the most recognisable production move of the last twenty years: a
//! kick pressing the pad and the bass down on every beat, so the low end is
//! taken in turns rather than fought over. [`crate::song`]'s mixer decides
//! which part is handed over and says why; this module only ever reads the one
//! it is given.

use super::gain::Ballistics;
use crate::level::{db_to_gain_f32, gain_to_db_f32};
use crate::stereo::Stereo;

/// The quietest a threshold may be set, in dBFS. Below this a compressor is
/// acting on the noise between the notes as hard as on the notes.
const MIN_THRESHOLD_DB: f32 = -60.0;

/// The loudest a threshold may be set, in dBFS. Full scale is where the
/// limiter's job starts, and holding a signal under the ceiling is that
/// device's promise rather than this one's.
const MAX_THRESHOLD_DB: f32 = 0.0;

/// The hardest ratio allowed. Past about 20:1 a compressor stops sliding the
/// level and starts holding a wall in front of it, which is a limiter — and
/// there is already one of those, at the only place it belongs.
const MAX_RATIO: f32 = 20.0;

/// The fastest either time constant may be, in seconds. About twenty samples:
/// shorter than one cycle of anything with a pitch, so the gain would be
/// tracking the waveform rather than the level and the result is distortion
/// rather than compression.
const MIN_TIME: f32 = 0.0005;

/// The slowest either time constant may be, in seconds. Two seconds is longer
/// than most phrases; past it the gain is a fader move rather than an effect.
const MAX_TIME: f32 = 2.0;

/// The most makeup allowed either way, in decibels. The same ceiling the EQ's
/// bands take, and for the same reason: past it this is not mixing.
const MAX_MAKEUP_DB: f32 = 24.0;

/// One compressor, with every setting already clamped to what it can honour.
///
/// A struct rather than six arguments because it is built once per chain entry
/// and then asked to run, and because the clamping has exactly one home that
/// way — the same shape [`super::eq`]'s `Biquad` takes.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Compressor {
    threshold_db: f32,
    ratio: f32,
    attack: f32,
    release: f32,
    makeup_db: f32,
    mix: f32,
}

impl Compressor {
    /// The settings as the document wrote them, clamped rather than refused —
    /// which is what every other fx parameter in this crate already does.
    pub(crate) fn new(
        threshold: f32,
        ratio: f32,
        attack: f32,
        release: f32,
        makeup: f32,
        mix: f32,
    ) -> Self {
        Self {
            threshold_db: finite(threshold, 0.0).clamp(MIN_THRESHOLD_DB, MAX_THRESHOLD_DB),
            ratio: finite(ratio, 1.0).clamp(1.0, MAX_RATIO),
            attack: finite(attack, MIN_TIME).clamp(MIN_TIME, MAX_TIME),
            release: finite(release, MIN_TIME).clamp(MIN_TIME, MAX_TIME),
            makeup_db: finite(makeup, 0.0).clamp(-MAX_MAKEUP_DB, MAX_MAKEUP_DB),
            mix: finite(mix, 1.0).clamp(0.0, 1.0),
        }
    }

    /// Compress `buf` in place, reading the level from `key` when there is one
    /// and from `buf` itself when there is not.
    ///
    /// The two are indexed from the same frame, because both are positions in
    /// the same piece. A key that runs out first stops ducking; one that runs
    /// on past the end of the signal has nothing left to duck.
    pub(crate) fn apply(&self, buf: &mut Stereo, key: Option<&Stereo>, rate: f32) {
        if self.is_bypass() || buf.is_empty() {
            return;
        }
        // Flushed before the detector reads them, so one poisoned sample is a
        // gap rather than a whole buffer of silence — the guard the limiter
        // and the EQ both keep.
        buf.each(|channel| {
            for sample in channel.iter_mut().filter(|s| !s.is_finite()) {
                *sample = 0.0;
            }
        });
        let gain = self.gain_track(key.unwrap_or(&*buf), buf.frames(), rate);
        let makeup = db_to_gain_f32(self.makeup_db);
        let (wet, dry) = (self.mix, 1.0 - self.mix);
        buf.each(|channel| {
            for (sample, gain) in channel.iter_mut().zip(&gain) {
                *sample = *sample * dry + *sample * gain * makeup * wet;
            }
        });
    }

    /// Whether this compressor is the identity, sample for sample.
    ///
    /// A fully dry one changes nothing, and so does a ratio of 1:1 with no
    /// makeup to add. Both are skipped rather than computed at unity, for the
    /// reason [`super::eq`]'s zero-gain bypass is exact: a bake is addressed
    /// by a hash, and "identical to within rounding" is a different file.
    fn is_bypass(&self) -> bool {
        self.mix <= 0.0 || (self.ratio <= 1.0 && self.makeup_db == 0.0)
    }

    /// The gain each of `frames` sample-frames is multiplied by: the static
    /// curve first, then given [`Ballistics`].
    fn gain_track(&self, key: &Stereo, frames: usize, rate: f32) -> Vec<f32> {
        let target: Vec<f32> = (0..frames)
            .map(|frame| self.target(level_at(key, frame)))
            .collect();
        Ballistics::new(self.attack, self.release, rate).smooth(&target)
    }

    fn target(&self, level: f32) -> f32 {
        if level <= 0.0 {
            return 1.0;
        }
        let over = gain_to_db_f32(level) - self.threshold_db;
        if over <= 0.0 {
            return 1.0;
        }
        db_to_gain_f32(-(over * (1.0 - 1.0 / self.ratio)))
    }
}

/// How loud the key is at one frame: the louder of its two channels, which is
/// what links them. Past its end it is silent, so a short key stops ducking
/// rather than repeating.
fn level_at(key: &Stereo, frame: usize) -> f32 {
    let side = |channel: &[f32]| channel.get(frame).copied().unwrap_or(0.0).abs();
    let (left, right) = (side(&key.l), side(&key.r));
    if left.is_finite() && right.is_finite() {
        left.max(right)
    } else {
        0.0
    }
}

/// `value`, or `fallback` when it is not a number.
fn finite(value: f32, fallback: f32) -> f32 {
    if value.is_finite() { value } else { fallback }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: f32 = 44_100.0;

    fn peak(buf: &[f32]) -> f32 {
        buf.iter().fold(0.0f32, |most, s| most.max(s.abs()))
    }

    /// A steady tone at `amplitude`, long enough for the ramps to settle.
    fn tone(amplitude: f32, frames: usize) -> Vec<f32> {
        (0..frames)
            .map(|i| amplitude * (std::f32::consts::TAU * 220.0 * i as f32 / RATE).sin())
            .collect()
    }

    fn four_to_one() -> Compressor {
        Compressor::new(-20.0, 4.0, 0.005, 0.05, 0.0, 1.0)
    }

    #[test]
    fn a_signal_over_the_threshold_comes_back_at_the_ratio() {
        // 0 dBFS in, 20 dB over a -20 dB threshold at 4:1, so 15 dB of that
        // excess is given back: −15 dBFS out, about 0.178.
        let mut buf = Stereo::centred(tone(1.0, 22_050));
        four_to_one().apply(&mut buf, None, RATE);
        let out = peak(&buf.l[11_025..]);
        assert!(
            (gain_to_db_f32(out) + 15.0).abs() < 1.0,
            "settled at {} dBFS",
            gain_to_db_f32(out)
        );
    }

    #[test]
    fn a_signal_under_the_threshold_is_handed_back_exactly() {
        let quiet = Stereo::centred(tone(0.05, 4410));
        let mut buf = quiet.clone();
        four_to_one().apply(&mut buf, None, RATE);
        assert_eq!(buf, quiet, "nothing crossed the threshold");
    }

    /// The linked detector, stated the way the limiter states it: a peak on
    /// one side moves both sides by the same amount, so the balance between
    /// them survives every moment the compressor acts.
    #[test]
    fn a_peak_on_one_side_pulls_both_down_together() {
        let mut buf = Stereo {
            l: tone(1.0, 8820),
            r: tone(0.25, 8820),
        };
        four_to_one().apply(&mut buf, None, RATE);
        let (left, right) = (peak(&buf.l[4410..]), peak(&buf.r[4410..]));
        assert!(
            (left / right - 4.0).abs() < 0.05,
            "the sides took different gains: {left} against {right}"
        );
    }

    /// The whole point of the sidechain: the gain follows a signal that is not
    /// the one being changed.
    #[test]
    fn a_key_ducks_a_signal_that_never_crosses_the_threshold_itself() {
        let mut key = Stereo::silence(8820);
        for slot in key.l[4410..5292].iter_mut().chain(&mut key.r[4410..5292]) {
            *slot = 1.0;
        }
        let steady = Stereo::centred(tone(0.05, 8820));
        let mut ducked = steady.clone();
        four_to_one().apply(&mut ducked, Some(&key), RATE);
        assert!(
            peak(&ducked.l[4410..5292]) < peak(&steady.l[4410..5292]) * 0.4,
            "the key never reached this signal"
        );
        assert_eq!(
            &ducked.l[..2205],
            &steady.l[..2205],
            "and well before the hit it is untouched"
        );
    }

    #[test]
    fn makeup_is_added_to_the_compressed_copy_and_a_parked_one_changes_nothing() {
        let original = Stereo::centred(tone(1.0, 8820));
        let mut loud = original.clone();
        Compressor::new(-20.0, 4.0, 0.005, 0.05, 12.0, 1.0).apply(&mut loud, None, RATE);
        let mut plain = original.clone();
        four_to_one().apply(&mut plain, None, RATE);
        assert!(
            (peak(&loud.l) / peak(&plain.l) - 4.0).abs() < 0.2,
            "12 dB up"
        );

        for parked in [
            Compressor::new(-40.0, 8.0, 0.005, 0.05, 6.0, 0.0),
            Compressor::new(-40.0, 1.0, 0.005, 0.05, 0.0, 1.0),
        ] {
            let mut buf = original.clone();
            parked.apply(&mut buf, None, RATE);
            assert_eq!(buf, original, "a bypass has to be exact");
        }
    }

    /// The lookahead, asserted on the gain track itself rather than inferred
    /// from the output — because "the peak came out quieter" is equally true of
    /// a compressor with no ramp at all, and the ramp is the whole difference
    /// between a duck and a step.
    ///
    /// Two claims, and each rules out one way of getting the other right by
    /// accident: the gain is **already moving** before the peak arrives, and it
    /// gets there **by a ramp** rather than by collapsing to nothing.
    #[test]
    fn the_gain_is_already_on_its_way_down_before_the_peak_arrives() {
        let mut key = Stereo::silence(2000);
        for slot in key.l[1000..].iter_mut().chain(&mut key.r[1000..]) {
            *slot = 1.0;
        }
        let gain = four_to_one().gain_track(&key, 2000, RATE);
        // A 5 ms attack is 220 samples, so nothing has moved 300 frames out.
        assert_eq!(gain[700], 1.0, "well before the peak, nothing has moved");
        assert!(gain[900] < 1.0, "100 frames out it is on its way down");
        assert!(
            gain[900] > gain[999] && gain[999] > gain[1000],
            "and still falling as the peak lands: {} {} {}",
            gain[900],
            gain[999],
            gain[1000]
        );
        assert!(
            gain[900] > gain[1000] * 2.0,
            "by a ramp, not by collapsing to nothing: {}",
            gain[900]
        );
    }

    /// The exactness the module doc claims for a parked compressor, at the
    /// setting that would give it away: at `mix` 0.3 a "bypass" computed as
    /// `0.7·s + 0.3·s` is *not* `s` in `f32`, so a compressor that ran its
    /// arithmetic at unity instead of standing aside would change the bytes of
    /// every bake carrying one — and a bake is addressed by a hash.
    #[test]
    fn a_parked_compressor_hands_back_the_signal_it_was_given() {
        let original = Stereo::centred(tone(0.7, 8820));
        let mut parked = original.clone();
        Compressor::new(-20.0, 1.0, 0.005, 0.05, 0.0, 0.3).apply(&mut parked, None, RATE);
        assert_eq!(parked, original, "1:1 with no makeup is not an effect");
    }

    /// Parallel — "New York" — compression: a hard-compressed copy under an
    /// untouched one, which is the whole reason `mix` is a field rather than
    /// always 1. Asserted sample by sample against the two copies it is made
    /// of, so neither side of the blend can be reaching the arithmetic
    /// sideways.
    #[test]
    fn a_partial_mix_is_the_two_copies_blended() {
        let original = Stereo::centred(tone(1.0, 8820));
        let at = |mix| Compressor::new(-20.0, 4.0, 0.005, 0.05, 6.0, mix);
        let mut squashed = original.clone();
        at(1.0).apply(&mut squashed, None, RATE);
        let mut blended = original.clone();
        at(0.25).apply(&mut blended, None, RATE);
        for (frame, ((dry, wet), out)) in original
            .l
            .iter()
            .zip(&squashed.l)
            .zip(&blended.l)
            .enumerate()
        {
            let expected = dry * 0.75 + wet * 0.25;
            assert!(
                (out - expected).abs() < 1e-6,
                "frame {frame}: {out} against {expected}"
            );
        }
        assert!(
            peak(&blended.l) > peak(&squashed.l),
            "three quarters of the signal never went through it"
        );
    }

    /// A key is another track's part, so unlike the signal it is not flushed on
    /// the way past and the guard has to be in the detector. A frame it cannot
    /// read is silence rather than a peak: read as one, an infinity asks for
    /// infinite reduction and punches a hole in a track that did nothing.
    #[test]
    fn a_frame_of_the_key_that_cannot_be_read_ducks_nothing() {
        let mut key = Stereo::silence(4410);
        key.l[2205] = f32::INFINITY;
        key.r[2205] = 0.5;
        let quiet = Stereo::centred(tone(0.05, 4410));
        let mut buf = quiet.clone();
        four_to_one().apply(&mut buf, Some(&key), RATE);
        assert_eq!(buf, quiet, "an unreadable frame is not a loud one");
    }

    #[test]
    fn nonsense_settings_and_an_empty_buffer_produce_no_nonsense() {
        let mut buf = Stereo {
            l: vec![f32::NAN, 0.9, f32::INFINITY, 0.9],
            r: vec![0.9, f32::NAN, 0.9, 0.9],
        };
        Compressor::new(f32::NAN, 4.0, -1.0, f32::NAN, f32::NAN, 4.0).apply(&mut buf, None, RATE);
        assert!(buf.l.iter().chain(&buf.r).all(|s| s.is_finite()), "{buf:?}");
        let mut empty = Stereo::silence(0);
        four_to_one().apply(&mut empty, None, RATE);
        assert!(empty.is_empty());
    }
}
