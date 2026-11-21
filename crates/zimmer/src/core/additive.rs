//! the additive series: a spectrum stated rather than carved.
//!
//! Every other head of the signal path *shapes*. [`osc`](super::osc) generates
//! something rich and hands it to a filter to carve down;
//! [`karplus`](super::karplus) excites a string and lets it ring;
//! [`fm`](super::fm) bends one sine with another and takes whatever sidebands
//! come out. This one says what the spectrum is: a table of partials, each
//! with a place, a weight and a fade, summed into one tone. A drawbar organ is
//! the literal case — the drawbars *are* the gains — and bowed and blown
//! sustains, glass and struck metal are the same idea with the partials
//! somewhere else.
//!
//! ## The two envelopes, and how they meet
//!
//! **A partial's `decay` and the patch's amp envelope multiply, and the
//! shorter of the two always wins.** This is the part of the source that is
//! easiest to get wrong, so it is stated rather than implied.
//!
//! The series is rendered at the head of the signal path, so what leaves this
//! module is already shaped: each partial has been scaled by
//! `exp(−t / decay)` before the filter, the amp envelope or anything else sees
//! it. The amp envelope then multiplies that whole sum. Two consequences
//! follow and neither is negotiable:
//!
//! - **The amp envelope cannot bring a partial back.** A partial whose own
//!   decay has taken it to nothing is gone for the rest of the note, however
//!   the envelope moves afterwards. That is the point — it is what makes a
//!   note brighten at its attack and settle into its body.
//! - **A partial's decay cannot outlast the note.** The amp envelope's release
//!   closes over everything, so a fundamental with a ten-second decay under an
//!   envelope that releases in 50 ms is a 50 ms note.
//!
//! So the two are doing different jobs, and a patch that gives them the same
//! job hears the note die twice: a percussive amp envelope over percussive
//! partial decays lands somewhere much shorter and duller than either was
//! written for. The shape to reach for is an amp envelope that **sustains** —
//! `{ "a": 0.01, "d": 0.0, "s": 1.0, "r": 0.2 }` — with the partial decays
//! carrying the movement, which is what an organ, a bowed string and a struck
//! bar all want.
//!
//! `decay` is measured from the **start of the note**, not from the gate
//! closing, exactly as [`fm`](super::fm)'s `mod_decay` is. A partial does not
//! know how long the key is held; it is a property of the tone.
//!
//! ## Partials above Nyquist are dropped, never folded
//!
//! A sixteenth partial of a note at 2 kHz sits at 32 kHz, which no 44.1 kHz
//! buffer can hold: it would fold back down to 12 kHz as inharmonic garbage
//! sitting inside the audible band. This is the problem polyBLEP solves for a
//! naive saw, and here the fix is simply not to render it.
//!
//! The decision is made **once per note** and against the *highest* frequency
//! the pitch track reaches — a partial is either in the note or out of it,
//! never switched off part-way through by a vibrato, which would be an audible
//! click at each crossing. It is per note rather than per patch because which
//! partials are legal depends on the pitch played: the same organ is a full
//! sixteen partials in the bass and a handful at the top of the keyboard,
//! which is also what a real one does.
//!
//! ## Normalised by the gain that sounds
//!
//! The sum is divided by the total gain of the partials that **actually
//! render**, so adding a partial thickens the tone without making it louder —
//! the rule [`osc`](super::osc) already follows for a stack. It also bounds
//! the output: with every partial's normalised gain summing to one, the worst
//! case is every one of them peaking together at exactly ±1.
//!
//! Dropped partials are left out of that total on purpose. Normalising against
//! the *declared* gains instead would make a note quieter the higher it is
//! played, purely because more of its series had fallen off the top — a source
//! whose level tracked the register would fight every gain decision the mix
//! makes, and a note high enough to be a lone fundamental would arrive as a
//! whisper rather than as a sine.
//!
//! ## Where each partial starts in its cycle
//!
//! Drawn from the note's seed, per partial, for the three reasons
//! [`osc`](super::osc)'s module doc sets out at length — and one more that is
//! specific here: partials all starting at phase zero peak together at the
//! first sample and nowhere else, which is a buzz with a click on it rather
//! than a tone. The draw costs nothing and takes nothing away, since it is a
//! pure function of `(seed, partial index)`, and the index is the partial's
//! place in the *declared* list, so dropping one at Nyquist does not move the
//! others.
//!
//! ## What it costs
//!
//! Sixteen partials is sixteen sine oscillators per note, which is more
//! arithmetic than any other source here. That claim deserved a number rather
//! than a shrug, so here is one: a whole second of one note through
//! `render_note` — source, amp envelope, allocation, interleave and all — on
//! the machine this is developed on (Ryzen 5 3400G), `--release`, averaged
//! over 200 runs.
//!
//! | source | ms per second of audio |
//! | --- | --- |
//! | one saw oscillator | 0.8 |
//! | four saw oscillators | 1.6 |
//! | `fm2` | 1.8 |
//! | four partials | 2.3 |
//! | sixteen partials | 8.0 |
//! | sixteen partials, every one decaying | 10.5 |
//!
//! So the worst case this source can be asked for is about **10 ms of CPU per
//! second of audio** — a real-time factor near 100×, and five to six times a
//! full oscillator stack. The last row is the `exp` per partial per sample
//! that a decay costs, which is why [`decay_at`] returns early rather than
//! evaluating `exp(0)`: an organ, where no partial decays, pays the 8 ms and
//! not the 10.5.
//!
//! `song/render.rs` already rests its voice allocator on the argument this
//! sits on, and it holds here: this is not a real-time synth, it is a buffer
//! being built. A three-minute piece with every one of its tracks on a
//! sixteen-partial series still spends a couple of seconds of its bake inside
//! this module.

use std::f32::consts::TAU;

use crate::core::nyquist;
use crate::hash::unit2;
use crate::patch::Partial;

/// Hash channel the start phases draw on, so a series never mirrors the noise
/// a `noise` source, a Karplus excitation or an oscillator stack draws from the
/// same note seed.
const PHASE_CHANNEL: u64 = 0x4144; // "AD"

/// Render the summed series for `freqs` (one base frequency per output sample)
/// into `out`.
///
/// `seed` is the note's, and decides only where each partial starts in its
/// cycle — see the module doc for why that is not zero.
pub(crate) fn render(
    partials: &[Partial],
    freqs: &[f32],
    seed: u64,
    out: &mut [f32],
    sample_rate: f32,
) {
    let ceiling = nyquist::ratio_ceiling(freqs, sample_rate);
    let total_gain: f32 = partials
        .iter()
        .filter(|partial| sounding(partial, ceiling).is_some())
        .map(|partial| partial.gain.max(0.0))
        .sum();
    let norm = if total_gain > 0.0 {
        1.0 / total_gain
    } else {
        0.0
    };
    for (index, partial) in partials.iter().enumerate() {
        let Some(ratio) = sounding(partial, ceiling) else {
            continue;
        };
        let gain = partial.gain.max(0.0) * norm;
        let mut phase = start_phase(index, seed);
        for (i, (s, base)) in out.iter_mut().zip(freqs).enumerate() {
            *s += gain * decay_at(i as f32 / sample_rate, partial.decay) * (TAU * phase).sin();
            phase = (phase + base * ratio / sample_rate).fract();
        }
    }
}

/// The frequency multiplier a partial's ratio and cent detune imply — when the
/// partial is one this note can carry at all.
///
/// `None` says it is not rendered: either it sits at or below DC, which is not
/// a pitch, or at or above Nyquist, where it would fold back into the note as
/// inharmonic garbage. Both comparisons are false for a `NaN` ratio, which is
/// the right answer for that too.
///
/// **Both bounds are exclusive, and the upper one deliberately so.** A partial
/// landing exactly on Nyquist advances exactly half a cycle per sample, so it
/// renders as an alternating ±A whose amplitude is decided by wherever in its
/// cycle the note happened to start — no converter can reproduce it and nobody
/// chose its level, which makes it the same unusable signal as one that folded.
fn sounding(partial: &Partial, ceiling: f32) -> Option<f32> {
    let ratio = partial.ratio * (partial.detune_cents / 1200.0).exp2();
    (ratio > 0.0 && ratio < ceiling).then_some(ratio)
}

/// How much of a partial is left at time `t`, under its own decay in seconds.
///
/// A `decay` of zero or less is a partial that does not fade on its own: it
/// holds its full weight for the whole note and leaves the shaping to the amp
/// envelope, which is what an organ wants and what a partial that does not
/// spell a decay out means.
#[inline]
fn decay_at(t: f32, decay: f32) -> f32 {
    if decay <= 0.0 {
        return 1.0;
    }
    (-t / decay).exp()
}

/// Where partial `index` of a note seeded `seed` starts in its cycle, in
/// `0..1`.
///
/// Per partial rather than per series: one draw shared across the series would
/// put every partial back in step, which is the phase-locked buzz the draw
/// exists to avoid.
fn start_phase(index: usize, seed: u64) -> f32 {
    unit2(index as i64, 0, PHASE_CHANNEL, seed)
}

/// The series by the numbers it claims: which partial is present, at what
/// level, and how it fades.
///
/// The assertions are spectral rather than aggregate, and deliberately so. A
/// test that an additive patch renders non-silent audio at roughly the right
/// pitch would pass with the gains ignored, the detune dropped, the Nyquist
/// rule inverted and the decay applied to the wrong partial. Every level below
/// is read out of the spectrum at the frequency it is claimed at.
///
/// **The window is chosen so every partial lands exactly on a bin.** The
/// fundamental is 100 Hz and the window is 4410 samples — a tenth of a second,
/// so partial `k` completes exactly `10k` cycles in it. That makes [`level_at`]
/// exact rather than approximate, and lets the tolerances be tight enough that
/// a wrong constant cannot hide inside one.
#[cfg(test)]
mod tests {
    use super::*;

    /// The played pitch every test below measures from.
    const BASE: f32 = 100.0;

    /// The rate everything renders at, as the DSP sees it.
    const RATE: f32 = 44_100.0;

    /// A tenth of a second: a whole number of cycles of [`BASE`] and of every
    /// harmonic of it, so the spectrum has no leakage to hide behind.
    const WINDOW: usize = 4410;

    fn partial(ratio: f32, gain: f32) -> Partial {
        Partial {
            ratio,
            gain,
            detune_cents: 0.0,
            decay: 0.0,
        }
    }

    /// The series rendered at [`BASE`] over `n` samples.
    fn series(partials: &[Partial], n: usize) -> Vec<f32> {
        let mut out = vec![0.0; n];
        render(partials, &vec![BASE; n], 11, &mut out, RATE);
        out
    }

    /// The amplitude of the sinusoid at `hz` in `buf`, by a one-bin DFT.
    ///
    /// Phase-invariant, which is what lets a level be asserted exactly even
    /// though every partial starts somewhere its seed chose.
    fn level_at(buf: &[f32], hz: f32) -> f32 {
        let (mut re, mut im) = (0.0f64, 0.0f64);
        for (i, s) in buf.iter().enumerate() {
            let phase = std::f64::consts::TAU * f64::from(hz) * i as f64 / f64::from(RATE);
            re += f64::from(*s) * phase.cos();
            im -= f64::from(*s) * phase.sin();
        }
        (2.0 * re.hypot(im) / buf.len() as f64) as f32
    }

    /// A partial is present at the frequency it names and at the level its
    /// gain claims, once the series has been normalised. Four partials at
    /// 8/4/2/1 make a 15-unit total, so the fundamental reads 8/15 and the
    /// fourth 1/15 — numbers no aggregate assertion could tell from any other
    /// weighting.
    #[test]
    fn each_partial_arrives_at_the_frequency_and_level_it_claims() {
        let buf = series(
            &[
                partial(1.0, 8.0),
                partial(2.0, 4.0),
                partial(3.0, 2.0),
                partial(4.0, 1.0),
            ],
            WINDOW,
        );
        for (ratio, gain) in [(1.0, 8.0), (2.0, 4.0), (3.0, 2.0), (4.0, 1.0)] {
            let want = gain / 15.0;
            let got = level_at(&buf, BASE * ratio);
            assert!(
                (got - want).abs() < 1e-3,
                "partial {ratio} read {got}, claimed {want}"
            );
        }
        // And nothing sits where no partial was placed.
        assert!(level_at(&buf, BASE * 5.0) < 1e-3);
    }

    /// The two things a magnitude spectrum cannot see, and so the two nothing
    /// in this file asserted until mutation testing said so: a partial is
    /// **added** into the buffer, and its phase walks **forward**.
    ///
    /// A one-bin DFT reports a magnitude, and a magnitude is blind to both.
    /// Negating the whole buffer leaves every level in it exactly where it
    /// was, and a sine whose phase runs backwards is a sine of the same
    /// frequency at the same amplitude — so every other assertion here passes
    /// with the sum inverted, the oscillator running in reverse, or both.
    ///
    /// Two samples of a lone partial settle it. Both expected values are
    /// worked out from what the module documents — a sine of `2π × phase`,
    /// starting at [`start_phase`] and advancing by `f / rate` a sample — and
    /// written as literals, because recomputing the renderer's own expression
    /// here would only assert that the code agrees with itself.
    ///
    /// - **Sample 0** is `sin(2π × 0.378687)`, the phase seed 11 draws for a
    ///   first partial. Its sign belongs to the accumulation: subtracting
    ///   rather than adding puts −0.6905 here, while running backwards does
    ///   not move it at all, since no step has been taken yet.
    /// - **Sample 110** is a hair past a quarter of a cycle at 100 Hz, which
    ///   is where the two directions are furthest apart: forward lands on
    ///   −0.7208 and backward on +0.7258, either side of zero.
    #[test]
    fn a_partial_is_added_in_and_walks_forward_through_its_cycle() {
        let buf = series(&[partial(1.0, 1.0)], 128);
        assert!(
            (buf[0] - 0.690_537).abs() < 1e-4,
            "sample 0 read {}",
            buf[0]
        );
        assert!(
            (buf[110] + 0.720_838).abs() < 1e-4,
            "sample 110 read {}, where backwards would read +0.7258",
            buf[110]
        );
    }

    /// Gains are weights and not levels: doubling every one of them changes
    /// nothing at all, sample for sample.
    #[test]
    fn the_series_is_normalised_by_the_gain_that_sounds() {
        let quiet = series(&[partial(1.0, 1.0), partial(2.0, 0.5)], 512);
        let loud = series(&[partial(1.0, 20.0), partial(2.0, 10.0)], 512);
        assert_eq!(quiet, loud);
    }

    /// Adding a partial thickens the tone without moving the level of the ones
    /// already there past what the normalisation says. One partial alone reads
    /// its full amplitude; beside an equal second, half of it.
    #[test]
    fn adding_a_partial_divides_the_level_rather_than_adding_to_it() {
        let alone = level_at(&series(&[partial(1.0, 1.0)], WINDOW), BASE);
        assert!((alone - 1.0).abs() < 1e-3, "a lone partial read {alone}");
        let shared = level_at(
            &series(&[partial(1.0, 1.0), partial(2.0, 1.0)], WINDOW),
            BASE,
        );
        assert!(
            (shared - 0.5).abs() < 1e-3,
            "beside a second it read {shared}"
        );
    }

    /// Whatever the series, the sum cannot leave `−1..=1`: the normalised
    /// gains total one, so the worst case is every partial peaking together.
    #[test]
    fn the_sum_stays_inside_unity() {
        let full: Vec<Partial> = (1..=16).map(|k| partial(k as f32, 1.0)).collect();
        let buf = series(&full, WINDOW);
        let peak = buf.iter().fold(0.0f32, |m, s| m.max(s.abs()));
        assert!(peak <= 1.0, "peaked at {peak}");
        assert!(peak > 0.2, "and is not silence ({peak})");
    }

    /// The Nyquist rule, at the frequency it would have folded to. A 16th
    /// partial of a 2 kHz note sits at 32 kHz and would alias to 12.1 kHz; it
    /// is dropped instead, so neither place has anything in it.
    #[test]
    fn a_partial_above_nyquist_is_absent_rather_than_folded() {
        let high = 2000.0;
        let mut out = vec![0.0; WINDOW];
        render(
            &[partial(1.0, 1.0), partial(16.0, 1.0)],
            &vec![high; WINDOW],
            11,
            &mut out,
            RATE,
        );
        assert!(level_at(&out, 32_000.0) < 1e-3, "nothing above Nyquist");
        assert!(level_at(&out, RATE - 32_000.0) < 1e-3, "and nothing folded");
        // Normalised over what sounds, so the survivor is at full level rather
        // than sharing the total with a partial nobody can hear.
        let fundamental = level_at(&out, high);
        assert!(
            (fundamental - 1.0).abs() < 1e-3,
            "the survivor read {fundamental}"
        );
    }

    /// The boundary itself — the half of the Nyquist rule the test above does
    /// not reach, since a sixteenth partial of a 2 kHz note is far past the
    /// line rather than sitting on it.
    ///
    /// A partial landing **exactly** on Nyquist is dropped. At half the sample
    /// rate a sine advances exactly half a cycle per sample, so what renders
    /// is an alternating ±A whose amplitude depends entirely on where in the
    /// cycle the note happened to start — a signal no converter can reproduce
    /// and nobody chose the level of, which is precisely what the rule is for.
    /// One part in ten thousand under the line still sounds.
    #[test]
    fn a_partial_landing_exactly_on_nyquist_is_dropped() {
        let ceiling = nyquist::ratio_ceiling(&[BASE], RATE);
        assert_eq!(ceiling, 220.5, "half the rate over the played pitch");
        assert_eq!(sounding(&partial(ceiling, 1.0), ceiling), None);
        assert!(sounding(&partial(220.49, 1.0), ceiling).is_some());
        // Were it kept, it would render at full level rather than not at all.
        let buf = series(&[partial(ceiling, 1.0)], 8);
        assert!(buf.iter().all(|s| *s == 0.0), "got {buf:?}");
    }

    /// It is decided per note, not per patch: the same series keeps a partial
    /// low down that it drops at the top of the keyboard.
    #[test]
    fn which_partials_are_legal_depends_on_the_note() {
        let ceiling = |hz: f32| nyquist::ratio_ceiling(&[hz], RATE);
        assert_eq!(ceiling(100.0), 220.5);
        assert_eq!(ceiling(2000.0), 11.025);
        assert!(sounding(&partial(16.0, 1.0), ceiling(100.0)).is_some());
        assert!(sounding(&partial(16.0, 1.0), ceiling(2000.0)).is_none());
    }

    /// A partial at or below DC names no frequency, so the renderer refuses it
    /// even though it can only arrive by calling past `Patch::validate`.
    #[test]
    fn a_partial_at_or_below_dc_is_not_rendered() {
        for ratio in [0.0, -1.0, f32::NAN] {
            assert!(sounding(&partial(ratio, 1.0), 220.5).is_none(), "{ratio}");
        }
        assert!(series(&[partial(0.0, 1.0)], 256).iter().all(|s| *s == 0.0));
    }

    /// The field the source exists for: an upper partial dies while the
    /// fundamental is still ringing. Measured in the spectrum of a late window
    /// against an early one, so it is the *partials* that are shown to part
    /// company rather than the overall level.
    #[test]
    fn an_upper_partial_can_die_before_the_fundamental() {
        let buf = series(
            &[
                Partial {
                    decay: 4.0,
                    ..partial(1.0, 1.0)
                },
                Partial {
                    decay: 0.02,
                    ..partial(4.0, 1.0)
                },
            ],
            22_050,
        );
        // Ten milliseconds at the front — one cycle of the fundamental and
        // four of the partial, so both still land on a bin, and short enough
        // that a 20 ms decay has not spent itself inside the window.
        let early = &buf[..441];
        let late = &buf[17_640..];
        assert!(
            level_at(early, BASE * 4.0) > 0.1,
            "the fourth starts audible"
        );
        assert!(level_at(late, BASE * 4.0) < 1e-3, "and is gone by the end");
        assert!(level_at(late, BASE) > 0.4, "while the fundamental rings on");
    }

    /// A decay of zero or less is a partial that holds — not one that is
    /// switched off, which is the neighbouring reading and the wrong one.
    #[test]
    fn a_partial_without_a_decay_holds_its_full_weight() {
        for decay in [0.0, -1.0] {
            assert_eq!(decay_at(0.0, decay), 1.0);
            assert_eq!(decay_at(10.0, decay), 1.0);
        }
        assert_eq!(decay_at(0.0, 0.5), 1.0, "a decay starts at full");
        // e⁻¹ at one time constant, and e⁻² at two: the shape by its numbers,
        // so a mutated sign or a dropped divide moves both.
        assert!((decay_at(0.5, 0.5) - 0.367_879_4).abs() < 1e-6);
        assert!((decay_at(1.0, 0.5) - 0.135_335_28).abs() < 1e-6);
    }

    /// Detune bends a partial off the harmonic grid by the cents it names —
    /// twelve hundred is an octave, so a partial at ratio 2 detuned by −1200
    /// lands on the fundamental.
    #[test]
    fn detune_moves_a_partial_in_cents() {
        let stretched = Partial {
            detune_cents: -1200.0,
            ..partial(2.0, 1.0)
        };
        let ratio = sounding(&stretched, 220.5).expect("it sounds");
        assert!((ratio - 1.0).abs() < 1e-5, "landed on {ratio}");
        assert_eq!(sounding(&partial(2.0, 1.0), 220.5), Some(2.0));
    }

    /// Each partial starts somewhere its own, each note starts somewhere its
    /// own, and either replays exactly.
    #[test]
    fn partials_do_not_start_in_step_and_each_note_still_replays() {
        assert_ne!(start_phase(0, 9), start_phase(1, 9));
        assert_ne!(start_phase(0, 9), start_phase(0, 10));
        for index in 0..MAX {
            let phase = start_phase(index, 9);
            assert!((0.0..1.0).contains(&phase), "phase {phase} is not a phase");
        }
        let struck = |seed: u64| {
            let mut out = vec![0.0; 2048];
            render(
                &[partial(1.0, 1.0), partial(3.0, 0.5)],
                &vec![BASE; 2048],
                seed,
                &mut out,
                RATE,
            );
            out
        };
        assert_eq!(struck(4), struck(4), "same seed, same samples");
        assert_ne!(struck(4), struck(5), "a second strike is not the first");
    }

    /// How many partials the phases are checked over: the cap, since that is
    /// the most a document can ask for.
    const MAX: usize = crate::patch::MAX_PARTIALS;

    /// `Patch::validate` refuses this series, so it can only arrive by calling
    /// the renderer directly — and then the normalisation must not divide by
    /// zero.
    #[test]
    fn a_series_with_no_gain_left_is_silence_not_a_divide_by_zero() {
        let silent = [partial(1.0, 0.0), partial(2.0, -1.0)];
        let buf = series(&silent, 512);
        assert!(buf.iter().all(|s| *s == 0.0), "got {:?}", &buf[..8]);
    }
}
