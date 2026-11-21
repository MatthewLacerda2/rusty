//! two-operator FM.
//!
//! One sine (the *modulator*) bends the frequency of another (the *carrier*). That
//! is the whole algorithm, and it buys the timbres subtractive synthesis is worst
//! at: electric pianos, bells, glassy plucks and metallic impacts.
//!
//! - `ratio` sets the modulator's frequency as a multiple of the played pitch.
//!   Whole-number ratios stay harmonic (tonal); ratios like `1.41` go inharmonic —
//!   that is where bells and hits live.
//! - `index` is the modulation depth: how many sidebands, i.e. how bright.
//! - `mod_decay` is the modulator's *own* exponential decay, in seconds. It is what
//!   makes an FM note read as *struck*: a bright transient collapsing to a near-sine
//!   body, the same way a real hammer or mallet behaves.
//!
//! Deliberately two operators, sines only. Six-operator FM is a synth in its own
//! right; this is the one extra source that pays for itself.
//!
//! **Both operators start where the note's seed says, and they are drawn
//! separately.** The argument is the oscillator stack's — see
//! [`osc`](crate::core::osc), which carries it in full, including why there is
//! deliberately no way to hard-sync a source and what the field would look like
//! the day a patch needs one. What is stronger here is the *second* draw. Two
//! operators sharing one start phase would begin at the same point in their
//! cycles on every note, and in two-operator FM the carrier-modulator phase
//! relationship **is** the timbre: it decides which sidebands the first
//! milliseconds are built from, which is the stretch of a bell or a tine the ear
//! identifies the sound by. One draw would move where the note starts without
//! moving that, so a repeated strike would still be the same attack — the change
//! would be cosmetic. Two draws move the relationship itself, which is what makes
//! a second hit a second hit.

use std::f32::consts::TAU;

use crate::hash::unit2;

/// Hash channel the two start phases draw on, so an FM voice never mirrors the
/// noise a `noise` source or a Karplus excitation draws, nor the cycle an
/// oscillator stack or an additive series starts in, from the same note seed.
const PHASE_CHANNEL: u64 = 0x464d; // "FM"

/// The lattice coordinate the carrier's start phase is drawn at.
const CARRIER: i64 = 0;

/// The modulator's, which is a different one for the reason the module doc
/// gives: it is their *relative* phase that is the timbre.
const MODULATOR: i64 = 1;

/// Render a two-operator FM note into `out`, following the per-sample frequency
/// track `freqs` (so an LFO vibrato applies to carrier and modulator alike).
///
/// `seed` is the note's, and decides only where the two operators start in
/// their cycles — see the module doc for why that is neither zero nor one draw
/// shared between them.
pub(crate) fn render(
    out: &mut [f32],
    freqs: &[f32],
    ratio: f32,
    index: f32,
    decay: f32,
    seed: u64,
    rate: f32,
) {
    let mut carrier = start_phase(CARRIER, seed);
    let mut modulator = start_phase(MODULATOR, seed);
    for (i, s) in out.iter_mut().enumerate() {
        let base = freqs.get(i).copied().unwrap_or(0.0);
        let env = mod_envelope(i as f32 / rate, decay);
        *s = (TAU * carrier + index * env * (TAU * modulator).sin()).sin();
        carrier = (carrier + base / rate).fract();
        modulator = (modulator + base * ratio / rate).fract();
    }
}

/// Where the operator at lattice coordinate `operator`, in a note seeded
/// `seed`, starts in its cycle, in `0..1`.
fn start_phase(operator: i64, seed: u64) -> f32 {
    unit2(operator, 0, PHASE_CHANNEL, seed)
}

/// The modulator's exponential decay at time `t`. A non-positive `mod_decay` means
/// "no bright transient at all", i.e. the carrier stays a plain sine.
#[inline]
fn mod_envelope(t: f32, decay: f32) -> f32 {
    if decay <= 0.0 {
        return 0.0;
    }
    (-t / decay).exp()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A note under the one seed every test here that is not about seeding
    /// uses, so a comparison between two renders is a comparison of the
    /// numbers being varied.
    fn render_note(ratio: f32, index: f32, decay: f32, n: usize) -> Vec<f32> {
        seeded_note(ratio, index, decay, n, 7)
    }

    fn seeded_note(ratio: f32, index: f32, decay: f32, n: usize, seed: u64) -> Vec<f32> {
        let mut out = vec![0.0; n];
        render(
            &mut out,
            &vec![220.0; n],
            ratio,
            index,
            decay,
            seed,
            44_100.0,
        );
        out
    }

    /// Sum of absolute sample-to-sample change — a cheap brightness proxy.
    fn roughness(buf: &[f32]) -> f32 {
        buf.windows(2).map(|w| (w[1] - w[0]).abs()).sum()
    }

    #[test]
    fn output_stays_inside_unity() {
        let buf = render_note(2.0, 8.0, 0.3, 44_100);
        assert!(buf.iter().all(|s| s.abs() <= 1.0 + 1e-6));
        assert!(buf.iter().any(|s| s.abs() > 0.5), "and is not silence");
    }

    #[test]
    fn a_higher_index_makes_a_brighter_tone() {
        let quiet = render_note(2.0, 0.5, 10.0, 4410);
        let bright = render_note(2.0, 8.0, 10.0, 4410);
        assert!(roughness(&bright) > roughness(&quiet) * 2.0);
    }

    #[test]
    fn zero_index_or_no_decay_collapses_to_a_plain_sine() {
        let plain = render_note(2.0, 0.0, 0.3, 4410);
        let no_decay = render_note(2.0, 8.0, 0.0, 4410);
        for (a, b) in plain.iter().zip(&no_decay) {
            assert!((a - b).abs() < 1e-6, "both must be the bare carrier");
        }
        // Exactly 22 periods of the carrier, so one rising crossing each
        // however far into its cycle the note started.
        let rising = plain
            .windows(2)
            .filter(|w| w[0] <= 0.0 && w[1] > 0.0)
            .count();
        assert_eq!(rising, 22, "4410 samples of a 220 Hz sine");
    }

    #[test]
    fn the_modulator_decays_so_the_note_dulls_as_it_rings() {
        let buf = render_note(3.0, 10.0, 0.05, 22_050);
        let attack = roughness(&buf[..2205]);
        let tail = roughness(&buf[19_845..]);
        assert!(
            attack > tail * 2.0,
            "attack {attack} should out-shine tail {tail}"
        );
        assert!((mod_envelope(0.0, 0.05) - 1.0).abs() < 1e-6);
        assert!(mod_envelope(0.5, 0.05) < 0.001);
    }
    /// The claim `generated/` rests on, at this source: a note is a pure
    /// function of what it was asked for, seed included.
    #[test]
    fn one_seed_renders_one_note_every_time() {
        let note = |seed| seeded_note(1.41, 6.0, 0.4, 4410, seed);
        assert_eq!(note(11), note(11), "an FM note is not reproducible");
    }

    /// And the point of drawing at all: the same note struck again is a second
    /// strike rather than a photocopy of the first.
    #[test]
    fn a_second_strike_is_not_the_first_one_over_again() {
        let note = |seed| seeded_note(1.41, 6.0, 0.4, 4410, seed);
        assert_ne!(note(11), note(12), "two strikes are the same samples");
    }

    /// The half of the draw that is not merely "somewhere in the cycle": the
    /// carrier and the modulator start apart, so what moves between two strikes
    /// is the relationship the timbre is made of and not only where the note
    /// begins. One draw shared between them would pass every test above.
    #[test]
    fn the_carrier_and_the_modulator_start_apart() {
        for seed in 0..32 {
            let (carrier, modulator) = (start_phase(CARRIER, seed), start_phase(MODULATOR, seed));
            assert_ne!(
                carrier, modulator,
                "one draw for two operators (seed {seed})"
            );
            for phase in [carrier, modulator] {
                assert!((0.0..1.0).contains(&phase), "phase {phase} is not a phase");
            }
        }
    }

    /// And the relationship itself is re-drawn per note rather than merely
    /// offset with it: the gap between the two operators is a different gap on
    /// the next strike, which is the whole of what the second draw buys.
    #[test]
    fn the_gap_between_the_operators_is_redrawn_per_note() {
        let gap = |seed| start_phase(MODULATOR, seed) - start_phase(CARRIER, seed);
        for seed in 0..32 {
            assert!(
                (gap(seed) - gap(seed + 1)).abs() > 1e-6,
                "seeds {seed} and {} hold the operators the same distance apart",
                seed + 1
            );
        }
    }
}
