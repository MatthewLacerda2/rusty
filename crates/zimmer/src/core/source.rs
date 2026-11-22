//! Every source but `noise` and a widened `osc` stack renders one waveform, identical in both
//! channels.

use super::{additive, fm, karplus, noise, osc, NoteContext};
use crate::patch::{Algorithm, Operator, Source, FM_OPERATORS};
use crate::stereo::Stereo;

pub(crate) fn render(
    source: &Source,
    freqs: &[f32],
    pulse: Option<&[f32]>,
    velocity: f32,
    note: NoteContext,
) -> Stereo {
    let NoteContext { seed, rate, .. } = note;
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
    one_waveform(source, freqs, velocity, note, &mut mono);
    Stereo::centred(mono)
}

fn one_waveform(source: &Source, freqs: &[f32], velocity: f32, note: NoteContext, out: &mut [f32]) {
    let NoteContext { seed, rate, .. } = note;
    match source {
        Source::Karplus {
            damping,
            brightness,
        } => {
            // The first frequency only: a string's delay line is fixed once it is plucked.
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
            fm::two::render(out, freqs, *ratio, depth, *mod_decay, note);
        }
        Source::Fm4 {
            algorithm,
            operators,
            vel_index,
        } => {
            let levels = fm_levels(*algorithm, operators, *vel_index, velocity);
            fm::four::render(out, freqs, *algorithm, operators, &levels, note);
        }
        Source::Additive { partials } => additive::render(partials, freqs, seed, out, rate),
        Source::Noise { .. } | Source::OscStack { .. } => {}
    }
}

/// Velocity adds to modulators only: a carrier's level is its mix share, which the amp envelope
/// already scales. Floored at zero: a negative index only mirrors the modulator, as bright.
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

    fn note(seed: u64) -> NoteContext {
        NoteContext {
            gate: 1.0,
            seed,
            rate: 44_100.0,
        }
    }

    fn render_kind(source: &Source) -> Stereo {
        render(source, &vec![220.0; 8192], None, 1.0, note(5))
    }

    fn saw_stack() -> Source {
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
        }
    }

    fn two_partials() -> Source {
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
        }
    }

    fn every_kind() -> [Source; 8] {
        [
            saw_stack(),
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
            two_partials(),
        ]
    }

    fn operator(ratio: f32) -> Operator {
        Operator {
            ratio,
            level: 1.0,
            feedback: 0.0,
            env: None,
        }
    }

    /// 1 for a waveform, normalised to it; coloured noise is scaled to white's RMS, so its peaks
    /// run higher.
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
            assert!(render(&source, &[], None, 1.0, note(1)).is_empty());
        }
    }

    #[test]
    fn velocity_moves_a_modulator_and_never_a_carrier() {
        let operators = [operator(1.0), operator(2.0), operator(3.0), operator(1.0)];
        let bright = |algorithm, velocity| {
            let source = Source::Fm4 {
                algorithm,
                operators,
                vel_index: 6.0,
            };
            render(&source, &vec![220.0; 4096], None, velocity, note(5)).l
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

    #[test]
    fn a_negative_index_is_floored_rather_than_mirrored() {
        let operators = [operator(1.0), operator(2.0), operator(3.0), operator(1.0)];
        assert_eq!(
            fm_levels(Algorithm::Chain, &operators, -9.0, 1.0),
            [0.0, 0.0, 0.0, 1.0]
        );
    }

    fn roughness(buf: &[f32]) -> f32 {
        buf.windows(2).map(|w| (w[1] - w[0]).abs()).sum()
    }
}
