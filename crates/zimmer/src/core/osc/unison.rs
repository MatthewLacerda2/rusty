//! One written oscillator, several sounding ones.

use crate::hash::unit2;
use crate::patch::Osc;

/// ASCII "OS": no other zimmer draw uses this hash channel.
const PHASE_CHANNEL: u64 = 0x4f53;

/// Floored at one: `Patch::validate` refuses zero, but a direct call would divide by it.
pub(super) fn voices(osc: &Osc) -> usize {
    osc.voices.max(1)
}

pub(super) fn voice_gain(osc: &Osc, norm: f32) -> f32 {
    osc.gain.max(0.0) * norm / voices(osc) as f32
}

pub(super) fn voice_detune(osc: &Osc, voice: usize) -> f32 {
    let voices = voices(osc);
    if voices < 2 {
        return 0.0;
    }
    osc.spread * (voice as f32 / (voices - 1) as f32 - 0.5)
}

/// In [`voice_detune`]'s order, lowest pitch left. Centred is `0.0`, which `pan_gains` makes exact
/// unity on both sides.
pub(super) fn voice_pan(osc: &Osc, voice: usize) -> f32 {
    let voices = voices(osc);
    let width = if osc.width.is_finite() {
        osc.width.clamp(0.0, 1.0)
    } else {
        0.0
    };
    if voices < 2 || width == 0.0 {
        return 0.0;
    }
    width * (2.0 * voice as f32 / (voices - 1) as f32 - 1.0)
}

pub(super) fn start_phase(index: usize, voice: usize, seed: u64) -> f32 {
    unit2(index as i64, voice as i64, PHASE_CHANNEL, seed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::patch::{Wave, MAX_VOICES};

    fn unison(voices: usize, spread: f32) -> Osc {
        Osc {
            wave: Wave::Saw,
            detune_cents: 0.0,
            gain: 1.0,
            octave: 0,
            voices,
            spread,
            width: 0.0,
            pulse_width: 0.5,
        }
    }

    #[test]
    fn a_detuned_pair_does_not_start_in_step() {
        assert_ne!(start_phase(0, 0, 9), start_phase(1, 0, 9));
        assert_ne!(start_phase(0, 0, 9), start_phase(0, 0, 10));
        for index in 0..4 {
            let phase = start_phase(index, 0, 9);
            assert!((0.0..1.0).contains(&phase), "phase {phase} is not a phase");
        }
    }

    #[test]
    fn every_unison_voice_starts_somewhere_else() {
        let phases: Vec<f32> = (0..MAX_VOICES).map(|v| start_phase(1, v, 9)).collect();
        for phase in &phases {
            assert!((0.0..1.0).contains(phase), "phase {phase} is not a phase");
        }
        for (at, phase) in phases.iter().enumerate() {
            for other in &phases[at + 1..] {
                assert_ne!(phase, other, "two voices start together in {phases:?}");
            }
        }
        // And the first voice starts where a lone oscillator does, so a one-voice entry renders the
        // same samples.
        assert_eq!(phases[0], unit2(1, 0, PHASE_CHANNEL, 9));
    }

    #[test]
    fn unison_voices_are_spread_evenly_around_the_written_pitch() {
        let cents: Vec<f32> = (0..MAX_VOICES)
            .map(|v| voice_detune(&unison(MAX_VOICES, 21.0), v))
            .collect();
        assert!((cents[0] + 10.5).abs() < 1e-4, "lowest at {}", cents[0]);
        assert!((cents[6] - 10.5).abs() < 1e-4, "highest at {}", cents[6]);
        assert!(
            cents[3].abs() < 1e-4,
            "the middle voice moved to {}",
            cents[3]
        );
        for pair in cents.windows(2) {
            assert!((pair[1] - pair[0] - 3.5).abs() < 1e-4, "uneven: {cents:?}");
        }
        assert!(
            cents.iter().sum::<f32>().abs() < 1e-3,
            "the spread is off centre: {cents:?}"
        );
        assert_eq!(voice_detune(&unison(1, 50.0), 0), 0.0);
        let pair = unison(2, 20.0);
        assert_eq!(
            [voice_detune(&pair, 0), voice_detune(&pair, 1)],
            [-10.0, 10.0]
        );
    }

    #[test]
    fn a_widened_oscillator_keeps_its_share_of_the_stack() {
        assert_eq!(voice_gain(&unison(1, 12.0), 1.0), 1.0);
        for count in 2..=MAX_VOICES {
            let each = voice_gain(&unison(count, 12.0), 1.0);
            assert!(
                (each * count as f32 - 1.0).abs() < 1e-6,
                "{count} voices at {each} each do not add up to one"
            );
        }
        assert_eq!(voice_gain(&unison(4, 12.0), 0.5), 0.125);
        assert_eq!(
            voice_gain(
                &Osc {
                    gain: -1.0,
                    ..unison(4, 12.0)
                },
                1.0
            ),
            0.0
        );
    }

    #[test]
    fn a_wide_unison_fans_out_across_the_field() {
        let wide = Osc {
            width: 1.0,
            ..unison(5, 20.0)
        };
        let pans: Vec<f32> = (0..5).map(|v| voice_pan(&wide, v)).collect();
        assert_eq!(pans, vec![-1.0, -0.5, 0.0, 0.5, 1.0]);
        let half = Osc {
            width: 0.5,
            ..unison(2, 20.0)
        };
        assert_eq!([voice_pan(&half, 0), voice_pan(&half, 1)], [-0.5, 0.5]);
        let over = Osc {
            width: 3.0,
            ..unison(3, 20.0)
        };
        assert_eq!(voice_pan(&over, 0), -1.0);
        for absurd in [f32::NAN, f32::INFINITY, -1.0] {
            let odd = Osc {
                width: absurd,
                ..unison(3, 20.0)
            };
            assert_eq!(voice_pan(&odd, 0), 0.0, "width {absurd} moved a voice");
        }
    }

    #[test]
    fn an_unspread_or_lone_voice_stays_centred() {
        for voice in 0..MAX_VOICES {
            assert_eq!(voice_pan(&unison(MAX_VOICES, 20.0), voice), 0.0);
        }
        let lone = Osc {
            width: 1.0,
            ..unison(1, 20.0)
        };
        assert_eq!(voice_pan(&lone, 0), 0.0);
    }

    #[test]
    fn an_oscillator_with_no_voices_still_sounds_once() {
        let none = unison(0, 12.0);
        assert_eq!(voices(&none), 1);
        assert_eq!(voice_gain(&none, 1.0), 1.0);
        assert_eq!(voice_detune(&none, 0), 0.0);
    }
}
