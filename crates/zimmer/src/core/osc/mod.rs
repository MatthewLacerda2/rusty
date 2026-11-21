//! the oscillator stack.
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
//! **One entry can be several oscillators.** `voices` turns an `Osc` into that
//! many copies of itself, spread evenly across `spread` cents and centred on
//! the detune it was written at — unison, and at seven voices on a saw it is
//! the supersaw. It lives on the oscillator rather than being written out as
//! separate stack entries for two reasons: it is one timbre, and spelling it
//! out costs the whole four-oscillator budget on a single sound, leaving
//! nothing for the sub-oscillator underneath it. The copies are normalised by
//! their own count, so widening an entry is a change of thickness and never of
//! level.
//!
//! Each oscillator keeps its own phase in `0..1`, advanced per sample by
//! `frequency / sample_rate`, so a per-sample frequency track (an LFO vibrato) needs
//! no special handling.
//!
//! **Where that phase starts is drawn from the note's seed, per oscillator and
//! per unison voice.**
//! Starting every one of them at zero instead is three problems wearing one
//! coat, and none of them announces itself. A detuned pair begins
//! locked, so the drift the detune exists for starts from the same instant
//! every time and the first tens of milliseconds, the part the ear identifies a
//! sound by, are a photocopy. Two hits of one patch are then literally the same
//! samples, which no struck instrument has ever managed. And notes stacked into
//! a chord reinforce and cancel at exactly the same points, which is a
//! comb-filtered chord rather than a chord.
//!
//! The draw costs nothing and takes nothing away: it is a pure function of
//! `(seed, oscillator index)` through [`crate::hash`], so one recipe still bakes
//! one file, and [`crate::song`] already hands each note in an arrangement its
//! own seed, so a repeat differs from the note it repeats without the document
//! saying a word.
//!
//! **There is deliberately no way to hard-sync it.** The case for one is a
//! percussive one-shot whose transient is its identity — a kick whose click
//! varies is worse, not better — and it does not survive contact with this
//! stack. Phase zero is not the quiet place a hard sync would want: a saw
//! starts at −1, a triangle and a square at their extremes, and only a sine
//! ever began at silence, so a drawn phase is on average *closer* to a clean
//! start than the locked one it replaces. A one-shot bake carries its own seed
//! and so is identical every time it is used regardless; the only place a
//! strike varies is inside a song, where the percussive sources — noise, and
//! the Karplus excitation — have redrawn per note since they were written. The
//! field to add, the day a patch genuinely wants a stated start, is a phase per
//! oscillator rather than a boolean, and it should arrive with that patch
//! rather than ahead of it.

pub(crate) mod pulse;
mod unison;

use std::f32::consts::TAU;

use crate::patch::{Osc, Wave};
use crate::stereo::{Stereo, pan_gains};
use unison::{start_phase, voice_detune, voice_gain, voice_pan, voices};

/// Render the summed stack for `freqs` (one base frequency per output sample) into
/// `out`. The mix is normalized by the total gain, so adding oscillators thickens
/// the tone without making it louder.
///
/// `seed` is the note's, and decides only where each oscillator and each of its
/// unison voices starts in its cycle — see the module doc for why that is not
/// zero.
///
/// **Stereo, because unison may be.** An oscillator's `width` places each of
/// its voices somewhere on the pan axis, through the same constant-power law a
/// track's `pan` uses, so the stack is the one source besides `noise` whose
/// two sides can differ. A centred voice is scaled by an exact `1.0` on both
/// sides, so a stack nobody widened writes the same samples into each channel
/// that a mono render would have, bit for bit.
///
/// `pulse`, when an LFO is aimed at the width, is how far every square's
/// [`Osc::pulse_width`] swings at each sample; every unison voice of an entry
/// shares the entry's width.
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

/// The frequency multiplier an oscillator's octave transpose and cent detune
/// imply, with `offset` more cents of unison detune on top.
fn pitch_ratio(osc: &Osc, offset: f32) -> f32 {
    (osc.octave as f32 + (osc.detune_cents + offset) / 1200.0).exp2()
}

/// One sample of `wave` at normalized `phase` (`0..1`), where `dt` is the phase
/// increment per sample (the polyBLEP correction needs it to size its window)
/// and `width` is the square's duty cycle, which no other wave reads.
fn sample(wave: Wave, phase: f32, dt: f32, width: f32) -> f32 {
    match wave {
        Wave::Sine => (TAU * phase).sin(),
        // A phase-shifted triangle: peaks at 0, troughs at half a cycle.
        Wave::Triangle => 4.0 * (phase - 0.5).abs() - 1.0,
        Wave::Saw => 2.0 * phase - 1.0 - poly_blep(phase, dt),
        Wave::Square => pulse::sample(phase, dt, width),
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

    /// The same oscillator, widened into `voices` copies `spread` cents apart.
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

    /// One side of a stack that nothing widened — and the proof that the
    /// other side is the same samples, which every caller of this relies on.
    fn render_stack(oscs: &[Osc], hz: f32, n: usize, seed: u64) -> Vec<f32> {
        let out = render_pair(oscs, hz, n, seed);
        assert_eq!(out.l, out.r, "an unwidened stack parted its channels");
        out.l
    }

    fn render_one(wave: Wave, hz: f32, n: usize) -> Vec<f32> {
        render_stack(&[osc(wave, 1.0, 0, 0.0)], hz, n, 7)
    }

    /// Rising zero-crossings in a buffer — one per cycle, wherever the cycle
    /// began.
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
        // 100 Hz over one second: 100 zero-crossings going positive, give or
        // take the one at the edge — the buffer no longer begins at phase zero,
        // so whether the hundredth crossing lands inside it depends on where in
        // its first cycle the oscillator started.
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
        // Gains are weights, not levels: doubling every one of them changes
        // nothing, and a second oscillator thickens the tone without making it
        // louder. Both halves are read off peaks rather than samples because
        // the two oscillators no longer start in step.
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

    /// The whole point of drawing the phase, and the promise it must not cost:
    /// two notes of one patch differ, and either of them replays exactly.
    #[test]
    fn a_second_strike_differs_but_each_one_replays() {
        let struck = |seed: u64| {
            let oscs = [osc(Wave::Saw, 1.0, 0, 0.0), osc(Wave::Saw, 1.0, 0, 7.0)];
            render_stack(&oscs, 220.0, 2048, seed)
        };
        assert_eq!(struck(4), struck(4), "same seed, same samples");
        assert_ne!(struck(4), struck(5), "a second strike is not the first");
    }

    /// The promise the count exists to keep: widening an entry thickens it
    /// without turning it up. Read off the peak, because unison voices beat
    /// against each other and a single sample says nothing.
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

    /// Unison is the detune doing the work, not the phases: seven voices
    /// genuinely at seven pitches beat against each other, so the sound's own
    /// envelope moves where a single oscillator's is flat. A `spread` that
    /// never reached the pitch would leave this dead level.
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

    /// Nothing about unison arriving may change what a patch without it
    /// renders to — including the spread, which a single voice has nothing to
    /// be spread against.
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

    /// The stack **adds** its oscillators into the buffer, and the sign of
    /// that accumulation is the one thing every other assertion in this file
    /// is blind to: peaks, rising-crossing counts and equality between two
    /// renders all survive negating the whole output, so `+=` could become
    /// `-=` and nothing here would notice.
    ///
    /// Read against the waveform the oscillator's own start phase implies,
    /// which is the only reading with a sign in it. The run is a third of a
    /// cycle wide so it cannot sit entirely on a zero crossing, and the guard
    /// at the end is what proves that: it has to contain a sample the mutation
    /// would move a long way, not one it leaves near zero either way.
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
        // `Patch::validate` refuses this stack, so it can only arrive by calling the
        // renderer directly — and then the normalization must not divide by zero.
        let oscs = [osc(Wave::Saw, 0.0, 0, 0.0), osc(Wave::Sine, -1.0, 0, 0.0)];
        let out = render_stack(&oscs, 440.0, 512, 1);
        assert!(out.iter().all(|s| *s == 0.0), "got {:?}", &out[..8]);
    }

    /// A wide unison is two different signals, one per side — not one signal
    /// turned up on one side — and its power is what the centred stack's was,
    /// because each voice is placed by a constant-power law.
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

    /// Pearson correlation of two equally long signals.
    fn correlate(a: &[f32], b: &[f32]) -> f32 {
        let dot = |x: &[f32], y: &[f32]| x.iter().zip(y).map(|(p, q)| p * q).sum::<f32>();
        dot(a, b) / (dot(a, a) * dot(b, b)).sqrt()
    }
}
