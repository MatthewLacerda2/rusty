//! the oscillator stack.

pub(crate) mod pulse;
mod unison;
mod wave;

use crate::patch::Osc;
use crate::stereo::{pan_gains, Stereo};
use unison::{start_phase, voice_detune, voice_gain, voice_pan, voices};
use wave::sample;

pub(crate) fn render(
    oscs: &[Osc],
    freqs: &[f32],
    pulse: Option<&[f32]>,
    seed: u64,
    out: &mut Stereo,
    sample_rate: f32,
) {
    let total_gain: f32 = oscs.iter().map(|o| o.gain.max(0.0)).sum();
    let norm = if total_gain > 0.0 {
        1.0 / total_gain
    } else {
        0.0
    };
    for (index, osc) in oscs.iter().enumerate() {
        let voices = voices(osc);
        let gain = voice_gain(osc, norm);
        for voice in 0..voices {
            let ratio = pitch_ratio(osc, voice_detune(osc, voice));
            let (left, right) = pan_gains(voice_pan(osc, voice));
            let mut phase = start_phase(index, voice, seed);
            let sides = out.l.iter_mut().zip(out.r.iter_mut());
            for (i, ((l, r), base)) in sides.zip(freqs).enumerate() {
                let dt = (base * ratio / sample_rate).clamp(0.0, 0.5);
                let swing = pulse.and_then(|p| p.get(i)).copied().unwrap_or(0.0);
                let s = gain * sample(osc.wave, phase, dt, pulse::width(osc, swing));
                *l += s * left;
                *r += s * right;
                phase = (phase + dt).fract();
            }
        }
    }
}

fn pitch_ratio(osc: &Osc, offset: f32) -> f32 {
    (osc.octave as f32 + (osc.detune_cents + offset) / 1200.0).exp2()
}

#[cfg(test)]
mod tests {
    use super::wave::poly_blep;
    use super::*;
    use crate::patch::Wave;
    use crate::patch::MAX_VOICES;

    fn osc(wave: Wave, gain: f32, octave: i32, detune_cents: f32) -> Osc {
        Osc {
            wave,
            detune_cents,
            gain,
            octave,
            voices: 1,
            spread: 12.0,
            width: 0.0,
            pulse_width: 0.5,
        }
    }

    fn unison(wave: Wave, voices: usize, spread: f32) -> Osc {
        Osc {
            voices,
            spread,
            ..osc(wave, 1.0, 0, 0.0)
        }
    }

    fn render_pair(oscs: &[Osc], hz: f32, n: usize, seed: u64) -> Stereo {
        let mut out = Stereo::silence(n);
        render(oscs, &vec![hz; n], None, seed, &mut out, 44_100.0);
        out
    }

    fn render_stack(oscs: &[Osc], hz: f32, n: usize, seed: u64) -> Vec<f32> {
        let out = render_pair(oscs, hz, n, seed);
        assert_eq!(out.l, out.r, "an unwidened stack parted its channels");
        out.l
    }

    fn render_one(wave: Wave, hz: f32, n: usize) -> Vec<f32> {
        render_stack(&[osc(wave, 1.0, 0, 0.0)], hz, n, 7)
    }

    fn rising(buf: &[f32]) -> usize {
        buf.windows(2).filter(|w| w[0] <= 0.0 && w[1] > 0.0).count()
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
    fn a_sine_completes_its_frequency_in_cycles() {
        let count = rising(&render_one(Wave::Sine, 100.0, 44_100));
        assert!((99..=100).contains(&count), "counted {count} cycles");
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
        // The naive ramp jumps a full 2.0 every cycle; the corrected one should not swing that hard
        // sample-to-sample at a high pitch (5 kHz, dt ≈ 0.11).
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
        let base = render_stack(&[osc(Wave::Sine, 1.0, 0, 0.0)], 200.0, n, 3);
        let down = render_stack(&[osc(Wave::Sine, 1.0, -1, 0.0)], 200.0, n, 3);
        assert!((199..=200).contains(&rising(&base)));
        assert!((99..=100).contains(&rising(&down)));
        assert!((pitch_ratio(&osc(Wave::Saw, 1.0, 0, 1200.0), 0.0) - 2.0).abs() < 1e-5);
        // Unison detune arrives in the same cents and adds to the written one.
        assert!((pitch_ratio(&osc(Wave::Saw, 1.0, 0, 600.0), 600.0) - 2.0).abs() < 1e-5);
    }

    #[test]
    fn the_stack_is_normalized_by_total_gain() {
        let loudest = |oscs: &[Osc]| {
            let out = render_stack(oscs, 440.0, 4410, 5);
            out.iter().fold(0.0f32, |m, s| m.max(s.abs()))
        };
        let one = loudest(&[osc(Wave::Sine, 1.0, 0, 0.0)]);
        assert!((one - 1.0).abs() < 0.05, "a lone sine peaks at {one}");
        assert_eq!(loudest(&[osc(Wave::Sine, 8.0, 0, 0.0)]), one);
        let both = loudest(&[osc(Wave::Sine, 1.0, 0, 0.0), osc(Wave::Sine, 1.0, 0, 0.0)]);
        assert!(both <= one + 1e-3, "two peaked at {both}, one at {one}");
    }

    #[test]
    fn a_second_strike_differs_but_each_one_replays() {
        let struck = |seed: u64| {
            let oscs = [osc(Wave::Saw, 1.0, 0, 0.0), osc(Wave::Saw, 1.0, 0, 7.0)];
            render_stack(&oscs, 220.0, 2048, seed)
        };
        assert_eq!(struck(4), struck(4), "same seed, same samples");
        assert_ne!(struck(4), struck(5), "a second strike is not the first");
    }

    #[test]
    fn unison_thickens_without_raising_the_level() {
        let loudest = |oscs: &[Osc]| {
            render_stack(oscs, 220.0, 22_050, 5)
                .iter()
                .fold(0.0f32, |m, s| m.max(s.abs()))
        };
        let one = loudest(&[unison(Wave::Saw, 1, 12.0)]);
        for voices in 2..=MAX_VOICES {
            let many = loudest(&[unison(Wave::Saw, voices, 18.0)]);
            assert!(
                many <= one + 1e-3,
                "{voices} voices peaked at {many}, one at {one}"
            );
            assert!(
                many > 0.3 * one,
                "{voices} voices all but cancelled ({many})"
            );
        }
    }

    #[test]
    fn detuned_voices_beat_against_each_other() {
        let swing = |osc: Osc| {
            let buf = render_stack(&[osc], 220.0, 44_100, 3);
            let peaks: Vec<f32> = buf
                .chunks_exact(2205)
                .map(|c| c.iter().fold(0.0f32, |m, s| m.max(s.abs())))
                .collect();
            let high = peaks.iter().copied().fold(0.0f32, f32::max);
            let low = peaks.iter().copied().fold(f32::MAX, f32::min);
            high - low
        };
        let steady = swing(unison(Wave::Sine, 1, 30.0));
        let beating = swing(unison(Wave::Sine, 5, 30.0));
        assert!(
            steady < 0.02,
            "a lone sine should hold level, swung {steady}"
        );
        assert!(
            beating > 0.15,
            "five detuned voices barely moved ({beating})"
        );
    }

    #[test]
    fn one_voice_is_the_oscillator_it_always_was() {
        let plain = render_stack(&[osc(Wave::Saw, 1.0, 0, 7.0)], 220.0, 4096, 11);
        for spread in [0.0, 12.0, 60.0] {
            let widened = Osc {
                spread,
                ..osc(Wave::Saw, 1.0, 0, 7.0)
            };
            assert_eq!(
                render_stack(&[widened], 220.0, 4096, 11),
                plain,
                "spread {spread} moved a single voice"
            );
        }
    }

    #[test]
    fn the_stack_adds_its_oscillators_rather_than_subtracting_them() {
        let (hz, seed) = (2205.0, 5);
        let out = render_stack(&[osc(Wave::Sine, 1.0, 0, 0.0)], hz, 7, seed);
        let dt = hz / 44_100.0;
        let start = start_phase(0, 0, seed);
        let mut loudest = 0.0f32;
        for (n, got) in out.iter().enumerate() {
            let want = sample(Wave::Sine, (start + dt * n as f32).fract(), dt, 0.5);
            assert!((got - want).abs() < 1e-5, "sample {n} is {got}, not {want}");
            loudest = loudest.max(want.abs());
        }
        assert!(loudest > 0.5, "every sample sat near zero ({loudest})");
    }

    #[test]
    fn a_stack_with_no_gain_left_is_silence_not_a_divide_by_zero() {
        // `Patch::validate` refuses this stack, so it can only arrive by calling the renderer
        // directly — and then the normalization must not divide by zero.
        let oscs = [osc(Wave::Saw, 0.0, 0, 0.0), osc(Wave::Sine, -1.0, 0, 0.0)];
        let out = render_stack(&oscs, 440.0, 512, 1);
        assert!(out.iter().all(|s| *s == 0.0), "got {:?}", &out[..8]);
    }

    #[test]
    fn a_wide_unison_puts_different_voices_on_each_side() {
        let centred = unison(Wave::Saw, 5, 25.0);
        let wide = Osc {
            width: 1.0,
            ..centred
        };
        let n = 44_100;
        let pair = render_pair(&[wide], 220.0, n, 3);
        let correlation = correlate(&pair.l, &pair.r);
        assert!(
            correlation < 0.6,
            "the two sides correlate at {correlation}: that is not width"
        );
        let power = |buf: &[f32]| buf.iter().map(|s| s * s).sum::<f32>();
        let flat = render_stack(&[centred], 220.0, n, 3);
        let ratio = (power(&pair.l) + power(&pair.r)) / (2.0 * power(&flat));
        assert!(
            (0.7..1.4).contains(&ratio),
            "widening moved the stack's power by {ratio}"
        );
    }

    fn correlate(a: &[f32], b: &[f32]) -> f32 {
        let dot = |x: &[f32], y: &[f32]| x.iter().zip(y).map(|(p, q)| p * q).sum::<f32>();
        dot(a, b) / (dot(a, a) * dot(b, b)).sqrt()
    }
}
