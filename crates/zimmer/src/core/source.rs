//! the source stage: which generator makes the tone
//!
//! One `match` over [`Source`], the head of the fixed signal path. Each arm hands
//! off to the module that owns that algorithm; nothing here knows any DSP. Sources
//! that can follow a moving pitch take the per-sample frequency track; Karplus-Strong
//! takes only the starting frequency, because its delay-line length is fixed once the
//! string is plucked.
//!
//! **This is where mono becomes stereo, and mostly it does not.** A plucked
//! string, an FM voice — of two operators or of four — and an additive series
//! are each one waveform, so each is rendered once and placed in both
//! channels, identically, which is what makes a centred part carry exactly the
//! samples it always did. Where a part sits is a decision the mix makes,
//! downstream, and a source that invented a position would take that decision
//! away from it.
//!
//! Two sources are exceptions, and each owns its reason: [`noise`], whose
//! second draw is free, and an [`osc`] stack whose unison was given a
//! `width` — several voices that are already different signals, which a wide
//! pad or a supersaw places across the field rather than in one spot (#1124).
//! An unwidened stack still writes identical channels.

use super::{additive, fm, karplus, noise, osc};
use crate::patch::{Algorithm, FM_OPERATORS, Operator, Source};
use crate::stereo::Stereo;

/// Render `source` following the per-sample frequency track `freqs`, one
/// sample-frame for each entry of it.
///
/// `pulse` is an LFO's per-sample swing of every square's pulse width, which
/// only an oscillator stack reads.
///
/// `velocity` is how hard the note was struck, already clamped to `0..=1` by
/// the caller — the *brightness* velocity, which a performance may scatter a
/// little either side of the one the fader got. Only FM reads it here — its
/// `vel_index` is depth a hard strike adds to the modulator, and depth is the
/// one place a source's *own* brightness is a number the patch already
/// carries. Every other source takes its velocity further down the path, at
/// the filter and the amp envelope.
///
/// `gate` is how long the note is held. Only `fm4` reads it here, for its
/// per-operator envelopes — every other source's shaping over time either
/// belongs to the note (the amp envelope, downstream) or is measured from the
/// start of it rather than from the gate closing.
///
/// `seed` is the note's, and **every** source draws on it: the noise source is
/// nothing but, the Karplus excitation is a burst of it, and an oscillator
/// stack, an additive series and an FM voice of either width each start their
/// voices somewhere in their cycle rather than all of them at zero — for the
/// stack that is per oscillator *and* per unison voice, for `fm2` the carrier
/// and the modulator drawn apart from each other, and for `fm4` all four
/// operators drawn apart, which is six relationships rather than one.
///
/// The sum is resolved here rather than inside [`fm`] so that module stays the
/// FM algorithm and nothing else: it is handed the index to use, not the
/// bookkeeping that arrived at one. `fm4` is the same rule over four
/// operators — which of them velocity reaches is a question the routing
/// answers, and [`fm_levels`] is where the two meet.
pub(crate) fn render(
    source: &Source,
    freqs: &[f32],
    pulse: Option<&[f32]>,
    seed: u64,
    velocity: f32,
    gate: f32,
    rate: f32,
) -> Stereo {
    let frames = freqs.len();
    match source {
        Source::Noise { color } => {
            let mut out = Stereo::silence(frames);
            noise::fill(&mut out, *color, seed);
            return out;
        }
        Source::OscStack { oscs } => {
            let mut out = Stereo::silence(frames);
            osc::render(oscs, freqs, pulse, seed, &mut out, rate);
            return out;
        }
        _ => {}
    }
    let mut mono = vec![0.0; frames];
    one_waveform(source, freqs, seed, velocity, gate, &mut mono, rate);
    Stereo::centred(mono)
}

/// The single waveform every source but `noise` produces.
fn one_waveform(
    source: &Source,
    freqs: &[f32],
    seed: u64,
    velocity: f32,
    gate: f32,
    out: &mut [f32],
    rate: f32,
) {
    match source {
        Source::Karplus {
            damping,
            brightness,
        } => {
            let freq = freqs.first().copied().unwrap_or(0.0);
            karplus::render(out, freq, *damping, *brightness, seed, rate);
        }
        Source::Fm2 {
            ratio,
            index,
            vel_index,
            mod_decay,
        } => {
            let depth = (index + vel_index * velocity).max(0.0);
            fm::two::render(out, freqs, *ratio, depth, *mod_decay, seed, rate);
        }
        Source::Fm4 {
            algorithm,
            operators,
            vel_index,
        } => {
            let levels = fm_levels(*algorithm, operators, *vel_index, velocity);
            let note = fm::four::Note { gate, seed, rate };
            fm::four::render(out, freqs, *algorithm, operators, &levels, note);
        }
        Source::Additive { partials } => additive::render(partials, freqs, seed, out, rate),
        // [`render`] sends these two down their own paths before narrowing
        // the signal to one channel: they are the sources that can draw two.
        Source::Noise { .. } | Source::OscStack { .. } => {}
    }
}

/// Each operator's level with velocity resolved into it: `vel_index` at full
/// strength added to every operator the routing makes a **modulator**, and
/// nothing at all added to a carrier.
///
/// The split is the whole point. A modulator's level is an index — depth in
/// radians, and therefore brightness — which is what a harder strike changes
/// on any real instrument. A carrier's level is its share of the mix, and
/// velocity already reaches the level through the amp envelope; adding to it
/// here would move the balance between the carriers as well, so a horn leaned
/// on would come out as a different horn rather than a brighter one.
///
/// Floored at zero for the reason `Fm2`'s `vel_index` is: a negative index
/// only mirrors the modulator and sounds exactly as bright, so an unclamped
/// sum would make a darkening routing brighten again once it crossed over.
fn fm_levels(
    algorithm: Algorithm,
    operators: &[Operator; FM_OPERATORS],
    vel_index: f32,
    velocity: f32,
) -> [f32; FM_OPERATORS] {
    std::array::from_fn(|i| {
        let level = operators[i].level;
        if algorithm.is_carrier(i) {
            level
        } else {
            (level + vel_index * velocity).max(0.0)
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::patch::{NoiseColor, Osc, Partial, Wave};

    fn render_kind(source: &Source) -> Stereo {
        render(source, &vec![220.0; 8192], None, 5, 1.0, 1.0, 44_100.0)
    }

    fn every_kind() -> [Source; 8] {
        [
            Source::OscStack {
                oscs: vec![Osc {
                    wave: Wave::Saw,
                    detune_cents: 0.0,
                    gain: 1.0,
                    octave: 0,
                    voices: 5,
                    spread: 20.0,
                    width: 0.0,
                    pulse_width: 0.5,
                }],
            },
            Source::Karplus {
                damping: 0.99,
                brightness: 0.5,
            },
            Source::Noise {
                color: NoiseColor::White,
            },
            Source::Noise {
                color: NoiseColor::Pink,
            },
            Source::Noise {
                color: NoiseColor::Brown,
            },
            Source::Fm2 {
                ratio: 2.0,
                index: 4.0,
                vel_index: 0.0,
                mod_decay: 0.3,
            },
            Source::Fm4 {
                algorithm: Algorithm::Twin,
                operators: [operator(2.0), operator(1.0), operator(3.0), operator(1.0)],
                vel_index: 0.0,
            },
            Source::Additive {
                partials: vec![
                    Partial {
                        ratio: 1.0,
                        gain: 1.0,
                        detune_cents: 0.0,
                        decay: 0.0,
                    },
                    Partial {
                        ratio: 2.0,
                        gain: 0.5,
                        detune_cents: 0.0,
                        decay: 0.0,
                    },
                ],
            },
        ]
    }

    /// A plain operator at `ratio`, full level, no feedback and no envelope.
    fn operator(ratio: f32) -> Operator {
        Operator {
            ratio,
            level: 1.0,
            feedback: 0.0,
            env: None,
        }
    }

    /// How loud a source may come out.
    ///
    /// One for everything with a waveform, which is normalised to it by
    /// construction. Coloured noise is the exception, and the number is its
    /// crest factor rather than a slackening: it is scaled to white's *RMS*,
    /// and a Gaussian-ish signal at the RMS of a uniform one peaks about twice
    /// as high. `crate::core::noise::color` argues that trade; the master
    /// limiter is where this crate answers for peaks.
    fn ceiling(source: &Source) -> f32 {
        match source {
            Source::Noise { color } if *color != NoiseColor::White => 3.0,
            _ => 1.0 + 1e-3,
        }
    }

    #[test]
    fn every_source_kind_produces_audible_finite_samples() {
        for source in every_kind() {
            let stereo = render_kind(&source);
            for buf in [&stereo.l, &stereo.r] {
                assert!(
                    buf.iter().all(|s| s.is_finite()),
                    "{source:?} went non-finite"
                );
                let peak = buf.iter().fold(0.0f32, |m, s| m.max(s.abs()));
                assert!(peak > 0.2, "{source:?} is inaudible (peak {peak})");
                assert!(peak <= ceiling(&source), "{source:?} peaked at {peak}");
            }
        }
    }

    /// The rule the module doc states, checked one source at a time: a
    /// generator makes one waveform and both channels get it, and `noise` is
    /// the single exception.
    #[test]
    fn only_noise_arrives_already_wide() {
        for source in every_kind() {
            let stereo = render_kind(&source);
            if matches!(source, Source::Noise { .. }) {
                assert_ne!(stereo.l, stereo.r, "noise draws each side its own");
            } else {
                assert_eq!(stereo.l, stereo.r, "{source:?} is one waveform, centred");
            }
        }
    }

    #[test]
    fn an_empty_frequency_track_does_not_panic() {
        for source in [
            Source::Noise {
                color: NoiseColor::Brown,
            },
            Source::Karplus {
                damping: 0.9,
                brightness: 0.5,
            },
        ] {
            assert!(render(&source, &[], None, 1, 1.0, 1.0, 44_100.0).is_empty());
        }
    }

    /// Velocity reaches the modulators and stops there. Under `chain` every
    /// operator but the last is a modulator, so a hard strike brightens the
    /// note; under `parallel` every one of them is a carrier, so the same
    /// `vel_index` — absurdly large, to leave no room for a small effect —
    /// changes nothing at all, sample for sample.
    #[test]
    fn velocity_moves_a_modulator_and_never_a_carrier() {
        let operators = [operator(1.0), operator(2.0), operator(3.0), operator(1.0)];
        let bright = |algorithm, velocity| {
            let source = Source::Fm4 {
                algorithm,
                operators,
                vel_index: 6.0,
            };
            render(
                &source,
                &vec![220.0; 4096],
                None,
                5,
                velocity,
                1.0,
                44_100.0,
            )
            .l
        };
        let (soft, hard) = (bright(Algorithm::Chain, 0.0), bright(Algorithm::Chain, 1.0));
        assert!(
            roughness(&hard) > roughness(&soft) * 2.0,
            "a harder strike should be brighter: {} against {}",
            roughness(&hard),
            roughness(&soft)
        );
        assert_eq!(
            bright(Algorithm::Parallel, 0.0),
            bright(Algorithm::Parallel, 1.0),
            "with no modulator there is nothing for velocity to reach"
        );
    }

    /// The levels a routing hands the renderer: `vel_index` on the three
    /// modulators of a chain, and the carrier left exactly as written.
    #[test]
    fn only_the_modulators_take_the_velocity_index() {
        let operators = [operator(1.0), operator(2.0), operator(3.0), operator(1.0)];
        assert_eq!(
            fm_levels(Algorithm::Chain, &operators, 4.0, 0.5),
            [3.0, 3.0, 3.0, 1.0]
        );
        assert_eq!(
            fm_levels(Algorithm::Twin, &operators, 4.0, 1.0),
            [5.0, 1.0, 5.0, 1.0]
        );
        assert_eq!(
            fm_levels(Algorithm::Parallel, &operators, 4.0, 1.0),
            [1.0, 1.0, 1.0, 1.0]
        );
    }

    /// A darkening routing bottoms out at a bare carrier rather than turning
    /// around and brightening again.
    #[test]
    fn a_negative_index_is_floored_rather_than_mirrored() {
        let operators = [operator(1.0), operator(2.0), operator(3.0), operator(1.0)];
        assert_eq!(
            fm_levels(Algorithm::Chain, &operators, -9.0, 1.0),
            [0.0, 0.0, 0.0, 1.0]
        );
    }

    /// Sum of absolute sample-to-sample change — a cheap brightness proxy.
    fn roughness(buf: &[f32]) -> f32 {
        buf.windows(2).map(|w| (w[1] - w[0]).abs()).sum()
    }
}
