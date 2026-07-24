//! src/soundgen/fx/limiter.rs — the peak limiter every bake passes through (#357).
//!
//! Not an effect the agent chooses: a bake **must never clip**. Detuned oscillators
//! drift in and out of phase, a resonant filter can double a peak, and summed notes
//! add up — any of which can push a buffer past ±1, where 16-bit quantization turns
//! the overshoot into a click. So the last thing a bake does is limit.
//!
//! Because we render offline we can do this properly, with **lookahead**: compute
//! the gain each sample needs, then walk the track backwards so the gain is already
//! down *before* a peak arrives (a short attack ramp instead of an instant duck),
//! and forwards so it recovers no faster than the release. Both passes only ever
//! lower the gain, so the ceiling is guaranteed, and a signal that never approaches
//! it is left untouched.

/// The peak the output is held under. A hair below full scale, so quantization
/// rounding cannot reach ±1.0.
const CEILING: f32 = 0.98;
/// Lookahead / attack ramp, in seconds — long enough to duck without a click.
const ATTACK: f32 = 0.002;
/// Recovery, in seconds. Slower than the attack, or the gain pumps audibly.
const RELEASE: f32 = 0.06;

/// Limit `buf` in place so no sample exceeds [`CEILING`]. Non-finite samples (from
/// a pathological patch) are flushed to silence rather than written to the file.
pub fn apply(buf: &mut [f32], rate: f32) {
    let mut gain = required_gain(buf);
    ramp_down_before_peaks(&mut gain, slope(ATTACK, rate));
    ramp_up_after_peaks(&mut gain, slope(RELEASE, rate));
    for (s, g) in buf.iter_mut().zip(&gain) {
        *s = (*s * g).clamp(-1.0, 1.0);
    }
}

/// The instantaneous gain each sample needs to sit under the ceiling — 1.0 wherever
/// the signal is already quiet enough. Sanitizes `buf` on the way past.
fn required_gain(buf: &mut [f32]) -> Vec<f32> {
    buf.iter_mut()
        .map(|s| {
            if !s.is_finite() {
                *s = 0.0;
            }
            let peak = s.abs();
            if peak > CEILING {
                CEILING / peak
            } else {
                1.0
            }
        })
        .collect()
}

/// Backward pass: the gain may fall no faster than `slope` per sample, so it is
/// already down when the peak arrives. This is what the lookahead buys.
fn ramp_down_before_peaks(gain: &mut [f32], slope: f32) {
    for i in (0..gain.len().saturating_sub(1)).rev() {
        gain[i] = gain[i].min(gain[i + 1] + slope);
    }
}

/// Forward pass: the gain may rise no faster than `slope` per sample, so it eases
/// back to unity instead of snapping.
fn ramp_up_after_peaks(gain: &mut [f32], slope: f32) {
    for i in 1..gain.len() {
        gain[i] = gain[i].min(gain[i - 1] + slope);
    }
}

/// Gain change allowed per sample for a ramp lasting `seconds`.
fn slope(seconds: f32, rate: f32) -> f32 {
    1.0 / (seconds * rate).max(1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::TAU;

    fn peak(buf: &[f32]) -> f32 {
        buf.iter().fold(0.0f32, |m, s| m.max(s.abs()))
    }

    #[test]
    fn a_hot_signal_is_brought_under_the_ceiling() {
        let mut buf: Vec<f32> = (0..44_100)
            .map(|i| 3.0 * (TAU * 220.0 * i as f32 / 44_100.0).sin())
            .collect();
        apply(&mut buf, 44_100.0);
        assert!(peak(&buf) <= CEILING + 1e-6, "peaked at {}", peak(&buf));
        assert!(peak(&buf) > 0.5, "but it is still a loud signal");
    }

    #[test]
    fn a_quiet_signal_passes_through_untouched() {
        let original: Vec<f32> = (0..1000)
            .map(|i| 0.5 * (TAU * 100.0 * i as f32 / 44_100.0).sin())
            .collect();
        let mut buf = original.clone();
        apply(&mut buf, 44_100.0);
        assert_eq!(
            buf, original,
            "a limiter must be transparent below its ceiling"
        );
    }

    #[test]
    fn the_gain_is_already_down_when_a_lone_peak_arrives() {
        // Silence, then one enormous spike: the samples just before it are quiet,
        // so the ducking is only visible as the spike itself being tamed.
        let mut buf = vec![0.0; 4410];
        buf[2205] = 10.0;
        apply(&mut buf, 44_100.0);
        assert!(buf[2205] <= CEILING + 1e-6);
        // The gain ramps back over the release, not instantly.
        let mut gain = required_gain(&mut [0.0, 10.0, 0.0, 0.0]);
        ramp_down_before_peaks(&mut gain, slope(ATTACK, 44_100.0));
        ramp_up_after_peaks(&mut gain, slope(RELEASE, 44_100.0));
        assert!(gain[0] < 1.0, "ducked before the peak");
        assert!(gain[2] < 1.0, "and recovers gradually after it");
    }

    #[test]
    fn non_finite_samples_are_flushed_to_silence() {
        let mut buf = vec![f32::NAN, f32::INFINITY, 0.5, f32::NEG_INFINITY];
        apply(&mut buf, 44_100.0);
        assert!(buf.iter().all(|s| s.is_finite()), "got {buf:?}");
        assert_eq!(buf[0], 0.0);
    }

    #[test]
    fn an_empty_buffer_is_a_no_op() {
        let mut buf: Vec<f32> = vec![];
        apply(&mut buf, 44_100.0);
        assert!(buf.is_empty());
    }
}
