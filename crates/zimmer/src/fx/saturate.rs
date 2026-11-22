//! Runs at the output rate, unoversampled, so it stays memoryless with no tail; bright sources
//! alias at high drive.

/// At the floor the curve leaves the identity by 1.3e-5, under half a 16-bit step; below, skipped.
const MIN_DRIVE: f32 = 0.01;

const MAX_DRIVE: f32 = 8.0;

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
        let mut buf = sine(1000.0);
        apply(&mut buf, 4.0, 1.0);
        // The 23rd harmonic, 23 kHz, is the first to fold: onto 21.1 kHz.
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
