//! Who the copies are: how many, where each one's sweep starts, how fast it runs, and where in the
//! field it sits.

use std::f32::consts::TAU;

use crate::hash::unit2;
use crate::stereo::pan_gains;

pub(super) const MIN_VOICES: usize = 2;

pub(super) const MAX_VOICES: usize = 4;

const RATE_SPREAD: f32 = 0.1;

const SPREAD_SEED: u64 = 0x0000_6368_6f72_7573;

const PHASE: u64 = 0;

const DEVIATION: u64 = 1;

pub(super) struct Voice {
    pub(super) phase: f32,
    pub(super) step: f32,
    pub(super) gains: (f32, f32),
}

pub(super) fn ensemble(count: usize, lfo_rate: f32, rate: f32) -> Vec<Voice> {
    let slice = TAU / count as f32;
    let mut voices: Vec<Voice> = (0..count)
        .map(|index| {
            let (ordinal, cell) = (index as f32, index as i64);
            let phase = (ordinal + unit2(cell, 0, PHASE, SPREAD_SEED)) * slice;
            Voice {
                phase,
                step: TAU * lfo_rate * rate_scale(cell) / rate,
                gains: pan_gains(-1.0 + 2.0 * ordinal / (count - 1) as f32),
            }
        })
        .collect();
    let left: f32 = voices.iter().map(|voice| voice.gains.0).sum();
    let right: f32 = voices.iter().map(|voice| voice.gains.1).sum();
    for voice in &mut voices {
        voice.gains = (voice.gains.0 / left, voice.gains.1 / right);
    }
    voices
}

fn rate_scale(cell: i64) -> f32 {
    let deviation = unit2(cell, 0, DEVIATION, SPREAD_SEED) * 2.0 - 1.0;
    1.0 + RATE_SPREAD * deviation
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: f32 = 44_100.0;

    #[test]
    fn the_voices_never_share_a_phase_or_a_rate() {
        for count in MIN_VOICES..=MAX_VOICES {
            let voices = ensemble(count, 1.0, RATE);
            assert_eq!(voices.len(), count);
            for (a, b) in voices.iter().zip(voices.iter().skip(1)) {
                assert!(b.phase > a.phase, "stratified, so strictly ordered");
                assert!((b.phase - a.phase) > 0.1, "and never near-coincident");
                assert_ne!(a.step, b.step, "each voice sweeps at its own rate");
            }
            let left: f32 = voices.iter().map(|v| v.gains.0).sum();
            assert!((left - 1.0).abs() < 1e-5, "the side sums to unity: {left}");
        }
    }

    #[test]
    fn each_voice_draws_its_phase_from_its_own_slice_of_the_cycle() {
        for count in MIN_VOICES..=MAX_VOICES {
            let slice = TAU / count as f32;
            for (index, voice) in ensemble(count, 1.0, RATE).iter().enumerate() {
                let floor = index as f32 * slice;
                assert!(
                    voice.phase >= floor && voice.phase < floor + slice,
                    "voice {index} of {count} is at {}, outside [{floor}, {})",
                    voice.phase,
                    floor + slice
                );
            }
        }
    }

    #[test]
    fn the_voices_are_spread_either_side_of_the_written_rate() {
        let (hz, count) = (2.0, MAX_VOICES);
        let nominal = TAU * hz / RATE;
        let voices = ensemble(count, hz, RATE);
        for voice in &voices {
            let off = voice.step / nominal - 1.0;
            assert!(
                off.abs() <= RATE_SPREAD + 1e-6,
                "{off} off the written rate"
            );
            assert!(off != 0.0, "and never exactly on it");
        }
        assert!(voices.iter().any(|v| v.step < nominal), "some run slower");
        assert!(voices.iter().any(|v| v.step > nominal), "and some faster");
        for (cell, voice) in voices.iter().enumerate() {
            let scaled = nominal * rate_scale(cell as i64);
            assert!(
                (voice.step - scaled).abs() < 1e-9,
                "voice {cell} of {count}"
            );
        }
    }

    #[test]
    fn the_ensemble_fills_the_field_from_one_edge_to_the_other() {
        for count in MIN_VOICES..=MAX_VOICES {
            let voices = ensemble(count, 1.0, RATE);
            assert_eq!(voices[0].gains.1, 0.0, "{count}: the first is hard left");
            assert_eq!(voices[count - 1].gains.0, 0.0, "and the last hard right");
            for (a, b) in voices.iter().zip(voices.iter().skip(1)) {
                assert!(b.gains.1 > a.gains.1, "{count}: each is further right");
                assert!(b.gains.0 < a.gains.0, "{count}: and less far left");
            }
        }
    }
}
