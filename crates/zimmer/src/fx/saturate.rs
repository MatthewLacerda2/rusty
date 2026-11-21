//! soft-clip drive — the one nonlinearity in the whole signal path.
//!
//! Everything else here is linear. Oscillators sum, the SVF is two integrators,
//! the amp envelope is a multiply, delay and reverb are sums of delayed copies, and
//! the limiter only ever scales. A linear chain can make a sound brighter, darker,
//! longer or louder, but it can never put a frequency into the output that was not
//! in the input — and that is precisely the property an ear recognises as *digital*.
//! Every record anyone has called warm, glued or expensive went through tape, a
//! tube, a transistor stage or a plugin imitating one, and all four are the same
//! thing underneath: a transfer curve that bends as the signal gets loud.
//!
//! The curve is `tanh`, which is what most "tape" and "tube" boxes are at heart —
//! smooth, odd-symmetric (so it makes odd harmonics, the musical ones), and with no
//! corner for a peak to catch on. One curve, deliberately: a second character is an
//! argument to have once a real recipe cannot be made with this one.
//!
//! ## Why `drive` is not a volume knob
//!
//! `tanh(drive · x)` on its own is quieter than `x` for a signal already near full
//! scale and louder for a quiet one, so turning it up would re-balance the mix and
//! every recipe using it would have to compensate by hand. Dividing by `tanh(drive)`
//! normalises the curve to pass through `(1, 1)`: a full-scale input comes back
//! full-scale at **every** drive, so `drive` changes the *shape* of the waveform and
//! not its peak. It has the two properties that matter at the ends, too — as `drive`
//! falls to zero the curve tends to the identity line, and it asymptotes at
//! `1/tanh(drive)` (1.04 at drive 2, 1.00 at drive 4), so anything hotter than full
//! scale comes back to about full scale. That is peaks handled gently *before* the
//! limiter has to act on them rather than after.
//!
//! That is peak-referenced, which is the right reference here because every place a
//! chain can live — a note, a track bus, the master sum — is upstream of the limiter
//! and sits near full scale by construction. What it does not promise is constant
//! *loudness*: a squarer wave of the same peak carries more energy, and that added
//! weight is the effect working rather than a level error.
//!
//! ## Aliasing, and the drive that stays clean
//!
//! A nonlinearity invents harmonics, and any harmonic landing above Nyquist folds
//! back down as an inharmonic tone — the same failure polyBLEP saves the naive saw
//! from, and one no waveshaper running at the output rate can avoid. `tanh` is soft
//! enough that its harmonics fall away fast, so what actually decides the damage is
//! how bright the *source* is, far more than how hard it is driven. Measured as
//! total folded energy against the fundamental, for a full-scale sine:
//!
//! | source | drive 1 | drive 2 | drive 4 | drive 8 |
//! | --- | --- | --- | --- | --- |
//! | 1 kHz | < −140 dB | −141 dB | −78 dB | −45 dB |
//! | 3 kHz | −88 dB | −53 dB | −31 dB | −21 dB |
//! | 5 kHz | −45 dB | −28 dB | −18 dB | −13 dB |
//!
//! So: on a bass, a drum, a pad or a whole mix — anything with its energy under a
//! couple of kHz — drive up to 4 is inaudibly clean. On a bright lead the honest
//! range is 1 to 2, and past that it will ring inharmonically. Running the shaper
//! 2× oversampled would buy 50–60 dB of that back, and is the exit if a recipe ever
//! genuinely needs a hard drive on a bright source; it is not done here because the
//! interpolation and decimation filters it needs are memory, and being memoryless is
//! what lets this effect claim a tail of exactly zero.

/// The smallest drive that is computed at all. At the floor itself the curve
/// departs from the identity line by 1.3 × 10⁻⁵ — a third of one 16-bit step —
/// so everything under it is a no-op worth skipping, which also keeps the
/// reciprocal of `tanh(drive)` away from the subnormals.
const MIN_DRIVE: f32 = 0.01;

/// The hardest push allowed. It is where the table above stops being reassuring:
/// past it the folded harmonics of any bright source sit within 20 dB of the
/// signal, and a soft clip that far in is a fuzz pedal — a different craft from
/// the weight this effect is here for.
const MAX_DRIVE: f32 = 8.0;

/// Waveshape `buf` in place.
///
/// `drive` is how hard the signal is pushed into the curve, gain-compensated so it
/// does not double as a fader; `mix` blends the shaped copy against the untouched
/// one, which is how a heavily driven layer sits under a clean one without losing
/// the transients. No `rate`: a waveshaper has no memory, so there is no sample
/// rate for it to have an opinion about.
pub(crate) fn apply(buf: &mut [f32], drive: f32, mix: f32) {
    if !drive.is_finite() || drive < MIN_DRIVE {
        return;
    }
    let drive = drive.min(MAX_DRIVE);
    let mix = mix.clamp(0.0, 1.0);
    let compensation = 1.0 / drive.tanh();
    for s in buf.iter_mut() {
        let wet = (*s * drive).tanh() * compensation;
        *s = *s * (1.0 - mix) + wet * mix;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::TAU;

    /// A tenth of a second of a full-scale sine. Every frequency the tests probe
    /// completes a whole number of cycles in it, so a magnitude is read straight
    /// off without a window.
    const N: usize = 4410;
    const RATE: f32 = 44_100.0;

    fn sine(freq: f32) -> Vec<f32> {
        (0..N)
            .map(|i| (TAU * freq * i as f32 / RATE).sin())
            .collect()
    }

    fn peak(buf: &[f32]) -> f32 {
        buf.iter().fold(0.0f32, |m, s| m.max(s.abs()))
    }

    /// Amplitude of the component at `freq`, by a single-bin DFT in `f64` so the
    /// arithmetic floor is far below anything being asserted about.
    fn magnitude_at(buf: &[f32], freq: f32) -> f64 {
        let w = std::f64::consts::TAU * f64::from(freq) / f64::from(RATE);
        let (re, im) = buf.iter().enumerate().fold((0.0, 0.0), |(re, im), (i, s)| {
            let (phase, s) = (w * i as f64, f64::from(*s));
            (re + s * phase.cos(), im + s * phase.sin())
        });
        2.0 * re.hypot(im) / buf.len() as f64
    }

    #[test]
    fn the_curve_invents_harmonics_that_were_not_in_the_source() {
        // The whole reason the effect exists: a pure sine has one partial, and a
        // saturated one does not.
        let clean = sine(1000.0);
        assert!(magnitude_at(&clean, 3000.0) < 1e-6, "a sine is a sine");
        let mut driven = clean.clone();
        apply(&mut driven, 1.0, 1.0);
        let third = magnitude_at(&driven, 3000.0) / magnitude_at(&driven, 1000.0);
        assert!(third > 0.05, "third harmonic is only {third} of the tone");
    }

    #[test]
    fn gain_compensation_leaves_a_full_scale_signal_full_scale() {
        for drive in [0.5, 1.0, 2.0, 4.0, 8.0] {
            let mut buf = sine(220.0);
            apply(&mut buf, drive, 1.0);
            assert!(
                (peak(&buf) - 1.0).abs() < 1e-3,
                "drive {drive} moved the peak to {}",
                peak(&buf)
            );
        }
    }

    #[test]
    fn anything_hotter_than_full_scale_is_pulled_back_toward_the_ceiling() {
        // Three times over, handed back at the curve's asymptote — the limiter
        // downstream now has almost nothing left to do.
        let mut buf: Vec<f32> = sine(220.0).iter().map(|s| s * 3.0).collect();
        apply(&mut buf, 2.0, 1.0);
        assert!(peak(&buf) < 1.05, "3.0 came back at {}", peak(&buf));
        assert!(peak(&buf) > 0.9, "but it is still a loud signal");
    }

    #[test]
    fn mix_blends_wet_against_dry() {
        let dry = sine(220.0);
        let mut wet = dry.clone();
        apply(&mut wet, 4.0, 1.0);
        let mut half = dry.clone();
        apply(&mut half, 4.0, 0.5);
        for ((h, d), w) in half.iter().zip(&dry).zip(&wet) {
            assert!((h - (d + w) * 0.5).abs() < 1e-6, "half of the way across");
        }
        let mut none = dry.clone();
        apply(&mut none, 4.0, 0.0);
        assert_eq!(none, dry, "a dry mix is not an approximation of the input");
    }

    #[test]
    fn the_documented_clean_range_really_is_clean() {
        // At 1 kHz the 23rd harmonic is the first one to fold, landing on
        // 21.1 kHz, where it is a harmonic of nothing and so is audible as
        // itself. The module table says drive 4 keeps it far down; this is that
        // claim, checked.
        let mut buf = sine(1000.0);
        apply(&mut buf, 4.0, 1.0);
        let folded = magnitude_at(&buf, 21_100.0) / magnitude_at(&buf, 1000.0);
        assert!(folded < 1e-3, "folded harmonic at {folded} of the tone");
    }

    #[test]
    fn a_degenerate_drive_is_a_no_op_rather_than_a_panic() {
        let original = sine(220.0);
        for drive in [0.0, -1.0, f32::NAN, f32::INFINITY, 1e-30] {
            let mut buf = original.clone();
            apply(&mut buf, drive, 1.0);
            assert_eq!(buf, original, "drive {drive} must change nothing");
        }
    }

    #[test]
    fn the_floor_is_where_computing_the_curve_starts_being_worth_it() {
        // Both sides of it, because the constant is a claim about audibility
        // and not a guess: at the floor the curve is computed and does bend the
        // signal, by well under one 16-bit step; below it nothing runs at all.
        let original = sine(220.0);
        let mut at_the_floor = original.clone();
        apply(&mut at_the_floor, MIN_DRIVE, 1.0);
        let moved = at_the_floor
            .iter()
            .zip(&original)
            .fold(0.0f32, |most, (a, b)| most.max((a - b).abs()));
        assert!(
            (1e-6..1e-4).contains(&moved),
            "the floor moved the signal by {moved}"
        );
        let mut under = original.clone();
        apply(&mut under, MIN_DRIVE * 0.9, 1.0);
        assert_eq!(under, original, "under the floor, nothing is computed");
    }

    #[test]
    fn drive_is_clamped_rather_than_taken_at_its_word() {
        let mut ceiling = sine(220.0);
        apply(&mut ceiling, MAX_DRIVE, 1.0);
        let mut absurd = sine(220.0);
        apply(&mut absurd, 1e6, 1.0);
        assert_eq!(absurd, ceiling, "past the ceiling is the ceiling");
    }
}
