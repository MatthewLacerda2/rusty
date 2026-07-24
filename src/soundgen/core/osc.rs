//! src/soundgen/core/osc.rs — the oscillator stack (#357).
//!
//! The head of a subtractive patch: up to four oscillators, each with its own wave,
//! detune, octave and gain, summed and normalized. Detuning two saws a few cents
//! apart is the entire trick behind a "fat" analog lead, so the stack is the one
//! place a patch gets width.
//!
//! Sine and triangle are generated naively — they are band-limited enough by their
//! own spectra. Saw and square are **polyBLEP** corrected: a naive ramp or step has
//! infinite harmonics, which fold back as audible inharmonic garbage above a few
//! hundred Hz. PolyBLEP subtracts a small polynomial around each discontinuity,
//! costs ~20 lines, and removes the worst of that aliasing.
//!
//! Each oscillator keeps its own phase in `0..1`, advanced per sample by
//! `frequency / sample_rate`, so a per-sample frequency track (an LFO vibrato) needs
//! no special handling.

use std::f32::consts::TAU;

use crate::soundgen::patch::{Osc, Wave};

/// Render the summed stack for `freqs` (one base frequency per output sample) into
/// `out`. The mix is normalized by the total gain, so adding oscillators thickens
/// the tone without making it louder.
pub fn render(oscs: &[Osc], freqs: &[f32], out: &mut [f32], sample_rate: f32) {
    let total_gain: f32 = oscs.iter().map(|o| o.gain.max(0.0)).sum();
    let norm = if total_gain > 0.0 {
        1.0 / total_gain
    } else {
        0.0
    };
    for osc in oscs {
        let ratio = pitch_ratio(osc);
        let gain = osc.gain.max(0.0) * norm;
        let mut phase = 0.0f32;
        for (s, base) in out.iter_mut().zip(freqs) {
            let dt = (base * ratio / sample_rate).clamp(0.0, 0.5);
            *s += gain * sample(osc.wave, phase, dt);
            phase = (phase + dt).fract();
        }
    }
}

/// The frequency multiplier an oscillator's octave transpose and cent detune imply.
fn pitch_ratio(osc: &Osc) -> f32 {
    (osc.octave as f32 + osc.detune_cents / 1200.0).exp2()
}

/// One sample of `wave` at normalized `phase` (`0..1`), where `dt` is the phase
/// increment per sample (the polyBLEP correction needs it to size its window).
fn sample(wave: Wave, phase: f32, dt: f32) -> f32 {
    match wave {
        Wave::Sine => (TAU * phase).sin(),
        // A phase-shifted triangle: peaks at 0, troughs at half a cycle.
        Wave::Triangle => 4.0 * (phase - 0.5).abs() - 1.0,
        Wave::Saw => 2.0 * phase - 1.0 - poly_blep(phase, dt),
        Wave::Square => {
            let naive = if phase < 0.5 { 1.0 } else { -1.0 };
            naive + poly_blep(phase, dt) - poly_blep((phase + 0.5).fract(), dt)
        }
    }
}

/// The polyBLEP residual: a two-sample parabolic smoothing of the step at phase 0,
/// which is what turns an aliasing discontinuity into a band-limited one.
fn poly_blep(phase: f32, dt: f32) -> f32 {
    if dt <= 0.0 {
        return 0.0;
    }
    if phase < dt {
        let t = phase / dt;
        2.0 * t - t * t - 1.0
    } else if phase > 1.0 - dt {
        let t = (phase - 1.0) / dt;
        t * t + 2.0 * t + 1.0
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn osc(wave: Wave, gain: f32, octave: i32, detune_cents: f32) -> Osc {
        Osc {
            wave,
            detune_cents,
            gain,
            octave,
        }
    }

    fn render_one(wave: Wave, hz: f32, n: usize) -> Vec<f32> {
        let mut out = vec![0.0; n];
        render(&[osc(wave, 1.0, 0, 0.0)], &vec![hz; n], &mut out, 44_100.0);
        out
    }

    #[test]
    fn every_wave_stays_inside_unity() {
        for wave in [Wave::Sine, Wave::Triangle, Wave::Saw, Wave::Square] {
            let buf = render_one(wave, 220.0, 4410);
            let peak = buf.iter().fold(0.0f32, |m, s| m.max(s.abs()));
            assert!(peak <= 1.0 + 1e-3, "{wave:?} peaked at {peak}");
            assert!(peak > 0.5, "{wave:?} is suspiciously quiet ({peak})");
        }
    }

    #[test]
    fn a_sine_completes_exactly_its_frequency_in_cycles() {
        // 100 Hz over one second: 100 zero-crossings going positive.
        let buf = render_one(Wave::Sine, 100.0, 44_100);
        let rising = buf.windows(2).filter(|w| w[0] <= 0.0 && w[1] > 0.0).count();
        assert_eq!(rising, 100);
    }

    #[test]
    fn poly_blep_only_touches_the_discontinuity() {
        assert_eq!(poly_blep(0.5, 0.01), 0.0, "mid-cycle is untouched");
        assert!(poly_blep(0.0, 0.01) < 0.0, "correction at the step");
        assert!(poly_blep(0.999, 0.01) > 0.0, "and just before it");
        assert_eq!(poly_blep(0.5, 0.0), 0.0, "a stopped oscillator needs none");
    }

    #[test]
    fn band_limiting_tames_a_high_saw() {
        // The naive ramp jumps a full 2.0 every cycle; the corrected one should not
        // swing that hard sample-to-sample at a high pitch (5 kHz, dt ≈ 0.11).
        let buf = render_one(Wave::Saw, 5000.0, 4410);
        let worst = buf
            .windows(2)
            .fold(0.0f32, |m, w| m.max((w[1] - w[0]).abs()));
        assert!(
            worst < 1.6,
            "saw still steps by {worst} — polyBLEP is not applied"
        );
    }

    #[test]
    fn octave_and_detune_shift_the_pitch() {
        // An octave-down oscillator crosses zero half as often.
        let n = 44_100;
        let mut base = vec![0.0; n];
        let mut down = vec![0.0; n];
        render(
            &[osc(Wave::Sine, 1.0, 0, 0.0)],
            &vec![200.0; n],
            &mut base,
            44_100.0,
        );
        render(
            &[osc(Wave::Sine, 1.0, -1, 0.0)],
            &vec![200.0; n],
            &mut down,
            44_100.0,
        );
        let count = |b: &[f32]| b.windows(2).filter(|w| w[0] <= 0.0 && w[1] > 0.0).count();
        assert_eq!(count(&base), 200);
        assert_eq!(count(&down), 100);
        assert!((pitch_ratio(&osc(Wave::Saw, 1.0, 0, 1200.0)) - 2.0).abs() < 1e-5);
    }

    #[test]
    fn the_stack_is_normalized_by_total_gain() {
        // Two unity-gain sines at the same pitch sum to a unity-peak sine, not two.
        let n = 4410;
        let mut out = vec![0.0; n];
        let oscs = [osc(Wave::Sine, 1.0, 0, 0.0), osc(Wave::Sine, 1.0, 0, 0.0)];
        render(&oscs, &vec![440.0; n], &mut out, 44_100.0);
        let peak = out.iter().fold(0.0f32, |m, s| m.max(s.abs()));
        assert!((peak - 1.0).abs() < 0.05, "peak {peak} is not normalized");
    }

    #[test]
    fn a_stack_with_no_gain_left_is_silence_not_a_divide_by_zero() {
        // `Patch::validate` refuses this stack, so it can only arrive by calling the
        // renderer directly — and then the normalization must not divide by zero.
        let n = 512;
        let mut out = vec![0.0; n];
        let oscs = [osc(Wave::Saw, 0.0, 0, 0.0), osc(Wave::Sine, -1.0, 0, 0.0)];
        render(&oscs, &vec![440.0; n], &mut out, 44_100.0);
        assert!(out.iter().all(|s| *s == 0.0), "got {:?}", &out[..8]);
    }
}
