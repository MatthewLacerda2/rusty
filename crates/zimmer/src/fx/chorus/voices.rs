//! Who the copies are: how many, where each one's sweep starts, how fast it
//! runs, and where in the field it sits.
//!
//! Split out from the effect itself because it answers a different question.
//! [`super`] is *what a modulated delay does to a signal*; this is *what the
//! ensemble is made of*, and it is the half that has to be deterministic —
//! every number here comes from the seeded hash, none of it from the audio.
//!
//! ## Stratified, not uniform
//!
//! Voice `v` takes its phase offset from inside the `v`-th slice of the cycle,
//! rather than from a draw over the whole of it. A uniform draw over four
//! voices will sometimes put two of them within a few degrees of each other,
//! and two voices in phase are not two voices — they are one voice 6 dB
//! louder, which is the exact failure a chorus exists to avoid. Stratifying
//! makes that unrepresentable instead of unlikely.
//!
//! ## The field, and what fills it
//!
//! The voices are placed evenly from one edge to the other with both ends
//! occupied, and each side's gains are then normalised to sum to one. Two
//! consequences worth stating, because both read the other way round:
//!
//! - **A voice count is not a fader.** The normalisation means the wet signal
//!   arrives at the level of the dry it is blended against, however many
//!   copies there are.
//! - **A voice count is not a width control either.** The width is carried by
//!   the outermost pair, so two panned hard is the widest an ensemble gets and
//!   four is the thickest — the ones between fill the field in rather than
//!   stretching it. More copies is more disagreement in the middle, which is
//!   what a section is.

use std::f32::consts::TAU;

use crate::hash::unit2;
use crate::stereo::pan_gains;

/// The fewest voices an ensemble is made of. Two: one copy is a detuned double
/// of the source with nowhere to be but beside it, and the field has two sides
/// to fill.
pub(super) const MIN_VOICES: usize = 2;

/// The most voices an ensemble is made of.
///
/// Four is already a section. Past it each added voice sits closer to one
/// already there — the ear stops counting copies somewhere around three — and
/// the cost is another interpolated line read per sample for a thickness
/// nobody can name. It is the argument [`crate::patch::MAX_OSCS`] makes about
/// a stack, applied to arithmetic that runs over every sample.
pub(super) const MAX_VOICES: usize = 4;

/// How far apart the voices' sweep rates are pulled, as a fraction of `rate`.
///
/// One shared rate gives an ensemble that breathes in unison, which is a
/// chorus pedal rather than a section. A tenth is enough that the voices drift
/// in and out of agreement over a few seconds without any of them running at a
/// noticeably different speed from the one written.
const RATE_SPREAD: f32 = 0.1;

/// The seed the phase and rate spreads are drawn under.
///
/// Fixed, and deliberately not the note's or the song's seed: the spread is a
/// property of *the effect*, so a chorus is the same ensemble on every note of
/// a part. Re-rolling it per note would make each note a differently-arranged
/// section, which is not what a section is.
///
/// The number is `chorus` in ASCII. Any constant would do — this one is only
/// so that a reader who wonders where it came from has an answer.
const SPREAD_SEED: u64 = 0x0000_6368_6f72_7573;

/// The hash channel a voice's phase offset is drawn on.
const PHASE: u64 = 0;

/// The hash channel a voice's rate deviation is drawn on, so it does not
/// correlate with that voice's phase.
const DEVIATION: u64 = 1;

/// One modulated tap: where its sweep is, how fast it moves, where it sits.
pub(super) struct Voice {
    /// Sweep phase in radians, advanced once per sample.
    pub(super) phase: f32,
    /// Radians per sample — this voice's own rate, spread off the written one.
    pub(super) step: f32,
    /// Left and right gains, already normalised across the ensemble.
    pub(super) gains: (f32, f32),
}

/// The voices, with their phases, rates and normalised placements resolved.
pub(super) fn ensemble(count: usize, lfo_rate: f32, rate: f32) -> Vec<Voice> {
    let slice = TAU / count as f32;
    let mut voices: Vec<Voice> = (0..count)
        .map(|index| {
            let (ordinal, cell) = (index as f32, index as i64);
            // Stratified: the v-th offset lands inside the v-th slice of the
            // cycle, so no two voices can draw their way into agreement.
            let phase = (ordinal + unit2(cell, 0, PHASE, SPREAD_SEED)) * slice;
            Voice {
                phase,
                step: TAU * lfo_rate * rate_scale(cell) / rate,
                // Evenly across the field, both ends included: the outermost
                // pair is what the width is carried by.
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

/// This voice's sweep rate as a multiple of the written one.
///
/// A draw from the seeded hash, mapped to `±`[`RATE_SPREAD`] around unity. Its
/// own function because the *sign* of the deviation is the one arbitrary thing
/// in this module: which voices come out fast and which slow is a property of
/// the hash and of nothing anybody decided, so it is the one place here a
/// mutation is genuinely equivalent. Named, that exclusion can name it — see
/// `.cargo/mutants.toml`.
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

    /// Stratification, stated as the property it is: voice `v` draws from
    /// inside the `v`-th slice of the cycle and nowhere else. This is the
    /// claim the module doc makes about why a uniform draw is not good
    /// enough, and it is exactly the kind that rots unasserted.
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

    /// Every voice sweeps near the written rate, none of them exactly at it,
    /// and they are pulled both ways — an ensemble that ran fast together
    /// would breathe in unison, which is the pedal this is not.
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
        // Each step is the written rate carrying *that* voice's scale. A
        // spread read only in aggregate cannot pin this: dividing by the scale
        // instead of multiplying by it gives a spread of much the same width,
        // made of the wrong voices.
        for (cell, voice) in voices.iter().enumerate() {
            let scaled = nominal * rate_scale(cell as i64);
            assert!(
                (voice.step - scaled).abs() < 1e-9,
                "voice {cell} of {count}"
            );
        }
    }

    /// The placement: the outermost pair is hard over on either side — which
    /// is what carries the width — and every voice between them sits further
    /// right than the last.
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
