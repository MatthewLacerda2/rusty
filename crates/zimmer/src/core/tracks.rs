//! The per-sample curves a note follows: pitch, cutoff, and the tremolo gain.
//!
//! One note is a buffer, and three of the things that shape it are decided
//! *per sample* rather than once: what frequency the source runs at, where the
//! filter sits, and how the level is dipped. Each is a `Vec<f32>` (or, for the
//! tremolo, a value read at an index) worked out before the stage that walks
//! it, which is what lets a filter stay a mono algorithm with mono state while
//! the curve it follows belongs to the note.
//!
//! **The rule they share is that sources of movement add.** A vibrato, a pitch
//! envelope and a glide are three terms of one sum in semitones; a filter
//! envelope, a velocity routing and a cutoff LFO are three terms of one sum in
//! octaves. Each is then applied *once*, as an exponent — so a zero term is
//! harmless, and no two of them can disagree about which applies first.

use super::RATE;
use super::env;
use super::lfo::Lfos;
use crate::note::Glide;
use crate::patch::{Filter, LfoTarget, PitchEnv};

/// The largest pitch offset the track will carry, either way: ten octaves.
///
/// A guard, not a musical limit, and the same kind as
/// [`MAX_SECONDS`](super::MAX_SECONDS). The
/// offset is an exponent, so an unbounded one reaches `inf` Hz, and an `inf`
/// frequency becomes a `NaN` sample the moment [`fm`] takes the fractional part
/// of a phase. Ten octaves either way is past the audible band from any note,
/// so nothing that was making a sound is affected by the bound existing.
///
/// It is nonetheless the **one thing in this crate's envelope work that a
/// recipe can notice**, and the reason [`SYNTH_VERSION`](crate::SYNTH_VERSION)
/// went to 2: a vibrato deeper than this renders differently than it used to.
/// It rendered aliasing before and renders aliasing now — but differently, and
/// the rule that constant lives by does not have a clause for *only garbage
/// moved*.
const MAX_PITCH_OFFSET: f32 = 120.0;

/// The per-sample frequency track: the played pitch, swept once by a pitch
/// envelope (`semitones` at full level), bent cyclically by an LFO aimed at
/// `pitch` (`depth` semitones either way), and slid onto from wherever a
/// [`Glide`] says the note starts.
///
/// They are summed **in semitones and then applied once**, rather than each
/// scaling the frequency in turn. That is the same "sources of movement add"
/// rule the cutoff already follows, and here it is also the only version that
/// is musically coherent: pitch is logarithmic, so semitones are what add, and
/// a sweep of −12 with a vibrato of ±1 is an octave drop with a semitone of
/// wobble on it however deep either one is.
///
/// **The glide is the one term that is not the patch's**, and it is shaped
/// differently for it: a straight line from `semitones` to zero over
/// `seconds`, and nothing at all after that. Linear, because a hand crosses a
/// distance at a speed and because linear *in semitones* is what an ear hears
/// as an even slide; and arriving exactly, because a note that never quite
/// reaches its own pitch is out of tune rather than expressive — which is what
/// an exponential approach, the other obvious shape, would give.
///
/// A note with none of the three takes the flat track, which is both faster
/// and the literal guarantee that a document written before any of these
/// stages existed still renders the frequency it always did.
pub(super) fn pitch_track(
    lfos: &Lfos,
    pitch_env: Option<PitchEnv>,
    glide: Option<Glide>,
    base: f32,
    gate: f32,
    n: usize,
) -> Vec<f32> {
    let vibrato = lfos.aims_at(LfoTarget::Pitch);
    let sweep = pitch_env.map(|p| (p.semitones, env::track(&p.adsr, gate, n, RATE)));
    // A slide of nothing, or one over no time, is not a slide: dropping it
    // here is what keeps the flat track available to a note whose mark had
    // nowhere to slide from.
    let slide = glide
        .filter(|g| g.semitones != 0.0 && g.seconds > 0.0)
        .map(|g| (g.semitones, g.seconds * RATE));
    if !vibrato && sweep.is_none() && slide.is_none() {
        return vec![base; n];
    }
    (0..n)
        .map(|i| {
            let mut semitones = 0.0;
            if vibrato {
                semitones += lfos.offset(LfoTarget::Pitch, i);
            }
            if let Some((amount, envelope)) = &sweep {
                semitones += amount * envelope[i];
            }
            if let Some((from, samples)) = slide {
                let left = 1.0 - i as f32 / samples;
                if left > 0.0 {
                    semitones += from * left;
                }
            }
            base * (semitones.clamp(-MAX_PITCH_OFFSET, MAX_PITCH_OFFSET) / 12.0).exp2()
        })
        .collect()
}

/// The per-sample cutoff track: the base cutoff, moved with the written note
/// `midi` (`key_track` octaves per octave from middle C), opened by how hard
/// the note was struck (`vel_octaves` at full velocity), swept by the filter envelope
/// (`env_octaves` at full level) and wobbled by an LFO aimed at `cutoff`
/// (`depth` octaves either way).
///
/// **All four depths are octaves, so all four are summed into one exponent**
/// and the base cutoff is multiplied by it once. That is the whole of the unit
/// change: the terms are still *added* to each other, so each source of
/// movement stays independent and a zero stays harmless, but the sum scales
/// the cutoff instead of offsetting it — three octaves is three octaves from
/// wherever the filter happens to sit, which is what the ear hears and what a
/// patch author can carry from one instrument to the next.
///
/// The LFO used to be the one term already written this way and was applied as
/// a separate multiply outside the sum; folding it in changes nothing about
/// what it does and removes the one place the two conventions met.
///
/// The key term reads `midi`, the written note, rather than the frequency
/// track: that track already carries the vibrato, which a cutoff should not
/// wobble with, so following a glide or a pitch envelope would cost a second
/// track.
///
/// `base` is where the filter sits at each sample when something outside the
/// note moves it — a song's `cutoff` curve, read across the whole note rather
/// than once at its onset (#1123) — and the patch's own `cutoff` when nothing
/// does. Every term above multiplies whichever it is.
///
/// Nothing here bounds the result — [`filter::apply`] clamps every cutoff into
/// the SVF's stable band anyway, and doing it twice would only invite the two
/// clamps to disagree. An absurd depth exponentiates to zero or to infinity
/// and both land on an end of that clamp.
pub(super) fn cutoff_track(
    f: &Filter,
    lfos: &Lfos,
    gate: f32,
    n: usize,
    velocity: f32,
    midi: f32,
    base: Option<&[f32]>,
) -> Vec<f32> {
    let envelope = env::track(&f.adsr, gate, n, RATE);
    let followed = f.key_track.clamp(0.0, 1.0) * (midi - 60.0) / 12.0;
    let struck = f.vel_octaves * velocity + followed;
    (0..n)
        .map(|i| {
            let octaves = struck + f.env_octaves * envelope[i];
            let wobble = lfos.offset(LfoTarget::Cutoff, i);
            let at = base.and_then(|base| base.get(i)).copied();
            at.unwrap_or(f.cutoff) * (octaves + wobble).exp2()
        })
        .collect()
}

#[cfg(test)]
mod key;

/// The pitch-bend path by the number, not the range.
///
/// What defends these two functions otherwise is a suite that asks whether a
/// note has audio in it and whether the level moves — which has no opinion
/// about *which* note is playing. A wrong operator here ships sound that is
/// wrong rather than sound that is absent, in the crate whose whole promise is
/// that one recipe always makes one file.
///
/// Every expected value below is worked out by hand from what the two
/// functions document and written as a literal; recomputing the formula in the
/// test would only assert that the code agrees with itself.
///
/// **On exactness.** These are `f32`, and each assertion says which it is:
/// `assert_eq!` where the maths is exact in binary — `sin(0)`, `2^0 = 1`,
/// `2^±1 = 2` and `0.5`, and the identity multiplies through them — and a tight
/// epsilon only where an irrational lands between two floats, with the reason
/// named. A tolerance wide enough to hide a swapped operator would defeat the
/// point of the file.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::patch::{Adsr, Lfo};

    /// The pitch every bend below is measured from: A4, and a round number of
    /// Hz, so an octave up and an octave down are round numbers too.
    const BASE: f32 = 440.0;

    /// The rate every wave below runs at. **Not 1 Hz**: `TAU * rate` and
    /// `TAU / rate` are the same expression at 1, so a rate of one would let
    /// that mutation through. 5 Hz also divides [`RATE`] evenly four ways,
    /// which is what makes the quarter-cycle index an integer.
    const RATE_HZ: f32 = 5.0;

    /// A quarter of a cycle at [`RATE_HZ`]: `44_100 / (4 * 5)`. The peak.
    const PEAK: usize = 2_205;

    /// Three quarters of a cycle: the trough.
    const TROUGH: usize = 6_615;

    /// A whole cycle, and one sample longer than the tracks need to be.
    const CYCLE: usize = 8_820;

    fn lfo(depth: f32, target: LfoTarget) -> Lfo {
        Lfo::new(RATE_HZ, depth, target)
    }

    /// `lfo`, or none, worked out over `n` frames of a note on its own.
    fn lfos(lfo: Option<Lfo>, n: usize) -> Lfos {
        Lfos::of(lfo.as_ref(), 0, None, n)
    }

    /// The frequency track a vibrato alone produces. The gate is a second —
    /// long enough to be irrelevant, since with no sweep nothing reads it.
    fn vibrato_track(lfo: Option<Lfo>, n: usize) -> Vec<f32> {
        pitch_track(&lfos(lfo, n), None, None, BASE, 1.0, n)
    }

    /// A pitch envelope of `semitones` under `adsr`.
    fn sweep(semitones: f32, adsr: Adsr) -> PitchEnv {
        PitchEnv { semitones, adsr }
    }

    /// The drum shape: full offset immediately, decaying to the played note
    /// over one second. Straight, so the level at a moment is a number this
    /// file can state rather than derive.
    fn falling() -> Adsr {
        Adsr {
            a: 0.0,
            d: 1.0,
            s: 0.0,
            r: 0.0,
            curve: 0.0,
        }
    }

    /// An envelope pinned at full level for the whole gate. Not a shape a drum
    /// wants, but the one that makes every sum below a round number.
    fn held() -> Adsr {
        Adsr {
            a: 0.0,
            d: 0.0,
            s: 1.0,
            r: 0.0,
            curve: 0.0,
        }
    }

    /// The frequency track a sweep alone produces, over a one-second gate so a
    /// sample index divided by [`RATE`] *is* the envelope's progress.
    fn sweep_track(semitones: f32, adsr: Adsr, n: usize) -> Vec<f32> {
        pitch_track(
            &lfos(None, n),
            Some(sweep(semitones, adsr)),
            None,
            BASE,
            1.0,
            n,
        )
    }

    /// A downward sweep starts high and arrives on the played note. Twenty-four
    /// semitones is two octaves, so half a straight decay is one octave and A4
    /// is A5 — both exact, because `2^2` and `2^1` are.
    ///
    /// The very first sample is the played pitch rather than the top of the
    /// sweep: [`env::level_at`] is zero at `t = 0` for every envelope there
    /// is, which is what keeps an amp envelope from starting with a step. One
    /// sample is 23 µs, so the sweep is audibly at its top from the start.
    #[test]
    fn a_pitch_envelope_starts_off_the_note_and_lands_on_it() {
        let track = sweep_track(24.0, falling(), 44_100);
        assert_eq!(track[0], BASE, "the envelope is still at zero");
        assert!(track[1] > 1_759.0, "two octaves up, at {}", track[1]);
        assert_eq!(track[22_050], 880.0, "one octave left at half the decay");
        assert!(
            (track[44_099] - BASE).abs() < 0.1,
            "and settles on the note, at {}",
            track[44_099]
        );
    }

    /// Negative sweeps the other way — up onto the note from below, not the
    /// same direction scaled.
    #[test]
    fn a_negative_amount_sweeps_up_onto_the_note_from_below() {
        let track = sweep_track(-12.0, held(), 8);
        assert_eq!(track[1], 220.0, "an octave below at full level");
    }

    /// The two modulators add **in semitones**: at the vibrato's peak the two
    /// twelves make two octaves, and at its trough they cancel exactly. A
    /// version that multiplied frequencies, or that let one replace the other,
    /// moves both numbers.
    #[test]
    fn a_sweep_and_a_vibrato_add_in_semitones() {
        let track = pitch_track(
            &lfos(Some(lfo(12.0, LfoTarget::Pitch)), CYCLE),
            Some(sweep(12.0, held())),
            None,
            BASE,
            1.0,
            CYCLE,
        );
        assert_eq!(track[PEAK], 1_760.0, "an octave of sweep plus one of bend");
        assert_eq!(track[TROUGH], BASE, "and the bend cancels the sweep");
    }

    /// A pitch envelope with no LFO at all still sweeps — the vibrato is not
    /// the thing that switches the modulated path on.
    #[test]
    fn a_sweep_needs_no_lfo_to_reach_the_track() {
        assert_eq!(sweep_track(12.0, held(), 8)[1], 880.0);
    }

    /// Ten octaves is the ceiling, so an amount past it cannot reach `inf` Hz —
    /// and an `inf` frequency is a `NaN` sample as soon as a phase is wrapped.
    #[test]
    fn an_absurd_sweep_is_clamped_instead_of_reaching_infinity() {
        for semitones in [1e9, -1e9, f32::MAX, f32::MIN] {
            let track = sweep_track(semitones, held(), 16);
            for (i, f) in track.iter().enumerate() {
                assert!(f.is_finite() && *f > 0.0, "{semitones} at {i} gave {f}");
            }
        }
    }

    /// The frequency track a glide alone produces, over a gate nothing reads.
    fn glide_track(semitones: f32, seconds: f32, n: usize) -> Vec<f32> {
        pitch_track(
            &lfos(None, n),
            None,
            Some(Glide { semitones, seconds }),
            BASE,
            1.0,
            n,
        )
    }

    /// A glide starts the note away from its own pitch and walks it back, and
    /// the **sign is which way**: an octave above starts at 880 and comes
    /// down, an octave below starts at 220 and comes up. A measurement of how
    /// far the pitch moved would read the same for both.
    #[test]
    fn a_glide_starts_off_the_note_in_the_direction_its_sign_says() {
        let above = glide_track(12.0, 0.1, 4_411);
        assert_eq!(above[0], 880.0, "an octave above, at the first sample");
        assert!((BASE..880.0).contains(&above[2_205]), "not on the way down");
        assert_eq!(above[4_410], BASE, "and not arrived");

        let below = glide_track(-12.0, 0.1, 4_411);
        assert_eq!(below[0], 220.0, "an octave below, at the first sample");
        assert!((220.0..BASE).contains(&below[2_205]), "not on the way up");
        assert_eq!(below[4_410], BASE, "and not arrived");
    }

    /// It arrives and then leaves the note alone — a way *into* a note rather
    /// than a detune it never comes back from.
    #[test]
    fn a_glide_stops_bending_the_note_once_it_has_arrived() {
        let track = glide_track(7.0, 0.01, 4_410);
        assert!(track[440] > BASE, "already given up with a sample to go");
        for (i, hz) in track.iter().enumerate().skip(441) {
            assert_eq!(*hz, BASE, "still bent at {i}");
        }
    }

    /// A slide of nowhere, and one over no time, are both not a slide.
    #[test]
    fn a_glide_of_nothing_bends_nothing() {
        for glide in [(0.0, 0.1), (12.0, 0.0), (12.0, -1.0)] {
            let track = glide_track(glide.0, glide.1, 8);
            assert_eq!(track, vec![BASE; 8], "{glide:?} bent the note");
        }
    }

    /// The glide is a third term of the same sum rather than a replacement for
    /// the other two: an octave of held sweep under an octave of slide is two
    /// octaves where they overlap, and one octave at either end where only one
    /// of them is doing anything.
    #[test]
    fn a_glide_adds_to_a_sweep_rather_than_replacing_it() {
        let track = pitch_track(
            &lfos(None, 0),
            Some(sweep(12.0, held())),
            Some(Glide {
                semitones: 12.0,
                seconds: 0.1,
            }),
            BASE,
            1.0,
            4_411,
        );
        assert_eq!(track[0], 880.0, "the slide alone, the envelope at zero");
        assert!((track[2_205] - 1_244.5).abs() < 0.1, "at {}", track[2_205]);
        assert_eq!(track[4_410], 880.0, "the sweep alone, the slide arrived");
    }

    /// The wave is zero at the first sample, so the bend is `2^0 = 1` and the
    /// pitch is the one that was played — for any depth at all, including one
    /// deep enough to be audible two octaves away.
    #[test]
    fn the_first_sample_is_the_played_pitch_whatever_the_depth() {
        for depth in [0.0, 2.0, 12.0, -7.0, 24.0] {
            let track = vibrato_track(Some(lfo(depth, LfoTarget::Pitch)), 8);
            assert_eq!(track[0], BASE, "depth {depth}");
        }
    }

    /// Twelve semitones is an octave: at the peak the bend is `2^(12/12) = 2`
    /// and A4 is A5, at the trough it is A3. Both numbers move under every
    /// mutation of the semitones-to-octaves divide, of the depth multiply and
    /// of the multiply that applies the bend to the pitch.
    #[test]
    fn twelve_semitones_bends_a_whole_octave_each_way() {
        let track = vibrato_track(Some(lfo(12.0, LfoTarget::Pitch)), CYCLE);
        assert_eq!(track[PEAK], 880.0);
        assert_eq!(track[TROUGH], 220.0);
    }

    /// Twice the depth is twice the octaves, not twice the frequency — the
    /// difference between a bend that is exponential in semitones and one that
    /// is anything else.
    #[test]
    fn twice_the_depth_is_twice_the_octaves() {
        let track = vibrato_track(Some(lfo(24.0, LfoTarget::Pitch)), CYCLE);
        assert_eq!(track[PEAK], 1760.0);
        assert_eq!(track[TROUGH], 110.0);
    }

    /// An LFO aimed at the filter or the amplifier does not touch the pitch,
    /// however deep it is — the whole track is the played note.
    #[test]
    fn an_lfo_aimed_elsewhere_leaves_the_pitch_flat() {
        for target in [LfoTarget::Cutoff, LfoTarget::Amp] {
            flat(&vibrato_track(Some(lfo(12.0, target)), CYCLE), CYCLE);
        }
    }

    /// And no LFO at all is the same flat track, at the length asked for.
    #[test]
    fn no_lfo_is_a_flat_track_of_the_length_asked_for() {
        flat(&vibrato_track(None, 512), 512);
    }

    /// `n` samples, every one of them the played pitch. Sample by sample
    /// rather than against a whole `Vec`, so a track that bends names the
    /// sample that first did instead of printing several thousand floats.
    fn flat(track: &[f32], n: usize) {
        assert_eq!(track.len(), n);
        for (i, f) in track.iter().enumerate() {
            assert_eq!(*f, BASE, "sample {i}");
        }
    }
}
