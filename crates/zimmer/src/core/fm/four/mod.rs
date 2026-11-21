//! four-operator FM: four sines, wired by the algorithm the recipe picked.
//!
//! [`two`](super::two) is one modulator on one carrier, which is one
//! relationship and so one kind of sound. Here the routing is chosen from
//! [`Algorithm`]'s table, and that choice is what opens the rest of FM: a
//! modulator with its own modulator, two modulators summed onto one carrier,
//! two whole voices mixed into one note.
//!
//! ## One pass, in order, and why that is the whole termination argument
//!
//! Every edge in the table runs from a lower-numbered operator to a
//! higher-numbered one, so a sample is produced by walking operators 1, 2, 3, 4
//! once: whatever modulates an operator was computed earlier in the same pass.
//! There is no iteration to converge and no graph to traverse — the routing
//! being a table rather than an edge list is what buys that, and
//! [`patch::fm`](crate::patch::fm) carries the argument for why it is a table.
//!
//! ## What an operator's level means
//!
//! A **modulator's** level is an index in radians, added into its target's
//! phase — the same quantity [`two`](super::two) calls `index`. A **carrier's**
//! level is a weight in the mix, and the carriers are normalised by their
//! total, so adding one thickens the tone rather than making it louder, the
//! rule an oscillator stack already follows. That
//! normalisation also bounds the output: the weights sum to one and each
//! carrier is a sine, so the worst case is every carrier peaking together at
//! exactly ±1.
//!
//! No operator is ever both under any row of the table, so no level ever means
//! two things at once.
//!
//! ## Feedback
//!
//! An operator may bend its own phase with its own last two output samples,
//! which is what reaches the rasping, growling end of the FM range. It is the
//! one place in the signal path where an output is read back as an input, so
//! the bound that keeps it from being the free-graph loop a patch may not
//! contain is stated where it lives, in [`feedback`](super::feedback). The
//! state it needs — two past samples per operator — is kept here, because it
//! belongs to the pass rather than to the arithmetic.
//!
//! ## Where the four start
//!
//! Not at zero, and not all together: each operator begins somewhere in its
//! cycle drawn from the note's own seed. [`phase`] carries the argument, which
//! is stronger here than at any other source in the crate — four operators
//! stand in six relationships, and in FM a relationship between operators is
//! not decoration on the timbre, it is the timbre.
//!
//! ## Above Nyquist: an operator is dropped, sidebands are not
//!
//! An operator whose own frequency is at or past half the sample rate is not
//! rendered at all. Its sine cannot be represented, so what it would
//! contribute is not a bright partial but an aliased one at some unrelated
//! frequency — as a carrier it would be an audible wrong note, and as a
//! modulator it would bend its target at a ratio nobody wrote. That question
//! is the additive source's too, so it is asked in one place —
//! [`nyquist`](crate::core::nyquist) — which also carries why it is decided
//! once per note against the top of the pitch track. Carriers that are dropped
//! leave the normalisation too, so a note played high does not also get
//! quieter.
//!
//! **The sidebands are a different matter and are not fixed here.** FM does not
//! synthesise its sidebands separately — they are what falls out of one `sin`
//! of a bent phase — so there is nothing to drop, and the ones above Nyquist
//! fold back into the audible band. Removing them would mean rendering the
//! source at several times the rate and filtering on the way down, which is a
//! cost every note would pay for a case a recipe can simply avoid. The bound
//! to write against is Carson's: a modulator of frequency `m` at index `i`
//! spreads a carrier roughly `(i + 1) × m` either side of itself, so keeping
//! `pitch × ratio × (index + 1)` under about 20 kHz keeps the fold-back
//! inaudible. High ratios and high indices belong on low notes, which is also
//! where they sound like anything.

mod phase;
mod routing;

use std::f32::consts::TAU;

use super::{feedback, voicing};
use crate::core::{env, nyquist};
use crate::patch::{Algorithm, FM_OPERATORS, Operator};

/// The note being played, as against the instrument playing it.
///
/// Three scalars that belong to *this* strike rather than to the patch: how
/// long it is held, which seed decides where its operators start, and the rate
/// it is rendered at. They travel together because [`render`]'s other four
/// arguments are the instrument, and because eight arguments in a row is a
/// signature nobody reads.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Note {
    /// How long the note is held, in seconds — what the per-operator
    /// envelopes are driven by.
    pub(crate) gate: f32,
    /// The note's seed. It decides one thing here: where the four operators
    /// start in their cycles. See [`phase`].
    pub(crate) seed: u64,
    /// Sample rate, in Hz.
    pub(crate) rate: f32,
}

/// Render four-operator FM into `out`, following the per-sample frequency
/// track `freqs`.
///
/// `levels` is each operator's level **with velocity already resolved into
/// it** — the caller does that, because how a velocity becomes an index is
/// bookkeeping and this module is the algorithm. `gate` is how long the note is
/// held, which is what the per-operator envelopes are driven by.
pub(crate) fn render(
    out: &mut [f32],
    freqs: &[f32],
    algorithm: Algorithm,
    operators: &[Operator; FM_OPERATORS],
    levels: &[f32; FM_OPERATORS],
    note: Note,
) {
    render_from(
        phase::starts(note.seed),
        out,
        freqs,
        algorithm,
        operators,
        levels,
        note,
    );
}

/// [`render`], from a stated set of start phases rather than the note's own
/// draw.
///
/// The seam exists because the two claims a magnitude spectrum cannot make —
/// that a modulator is *added* into a phase and that a phase walks *forward* —
/// are made against hand-computed samples, and a hand-computed sample needs to
/// know where the walk began. Everything above this line is the pass; the draw
/// is [`render`]'s one job.
fn render_from(
    mut phase: [f32; FM_OPERATORS],
    out: &mut [f32],
    freqs: &[f32],
    algorithm: Algorithm,
    operators: &[Operator; FM_OPERATORS],
    levels: &[f32; FM_OPERATORS],
    note: Note,
) {
    let Note { gate, rate, .. } = note;
    let ceiling = nyquist::ratio_ceiling(freqs, rate);
    let sounding = voicing::sounding(operators, ceiling);
    let norm = voicing::normaliser(algorithm, levels, &sounding);
    let mut fed = [[0.0f32; 2]; FM_OPERATORS];
    for (i, s) in out.iter_mut().enumerate() {
        let t = i as f32 / rate;
        let base = freqs.get(i).copied().unwrap_or(0.0);
        let mut shaped = [0.0f32; FM_OPERATORS];
        let mut mix = 0.0;
        for op in 0..FM_OPERATORS {
            if !sounding[op] {
                continue;
            }
            let operator = &operators[op];
            let bend = routing::modulation(algorithm.modulators(op), levels, &shaped)
                + feedback::bend(operator.feedback, fed[op]);
            shaped[op] = envelope(operator, t, gate) * (TAU * phase[op] + bend).sin();
            fed[op] = [shaped[op], fed[op][0]];
            if algorithm.is_carrier(op) {
                mix += levels[op].max(0.0) * shaped[op];
            }
            phase[op] = (phase[op] + base * operator.ratio / rate).fract();
        }
        *s = mix * norm;
    }
}

/// An operator's own envelope at time `t`, or a flat full level for an operator
/// that does not carry one.
#[inline]
fn envelope(operator: &Operator, t: f32, gate: f32) -> f32 {
    match &operator.env {
        Some(adsr) => env::level_at(adsr, t, gate),
        None => 1.0,
    }
}

/// The pass by what it puts in a *sample*: the feedback by its bound, the
/// envelopes by the fact that they are per operator, and the two signs a
/// spectrum cannot see.
///
/// A magnitude is blind to a sign flip and to a phase running backwards, so a
/// file of nothing but spectral assertions would pass with the whole buffer
/// negated or every oscillator running in reverse.
/// [`the_carrier_is_added_in_and_walks_forward`] closes both against
/// hand-computed samples instead, which is why there is a
/// [`render_from`] to hand stated phases to: a literal worked out by hand has
/// to know where the walk began. [`routing`] holds the spectral half, and
/// [`phase`] the draw itself.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::patch::Adsr;

    /// The played pitch every test below measures from.
    const BASE: f32 = 100.0;

    /// The rate everything renders at, as the DSP sees it.
    const RATE: f32 = 44_100.0;

    /// A tenth of a second: a whole number of cycles of [`BASE`] and of every
    /// harmonic of it.
    const WINDOW: usize = 4410;

    /// A gate longer than any window here, so no operator envelope is in its
    /// release unless a test puts it there.
    const HELD: f32 = 10.0;

    /// The one seed every test that is not about seeding plays under, so a
    /// comparison between two renders is a comparison of the numbers varied.
    const SEED: u64 = 7;

    fn op(ratio: f32, level: f32) -> Operator {
        Operator {
            ratio,
            level,
            feedback: 0.0,
            env: None,
        }
    }

    /// `operators` rendered under `algorithm` at `hz`, over `n` samples, with
    /// each operator's level taken as written and the note played under
    /// [`SEED`].
    fn voice(algorithm: Algorithm, operators: [Operator; 4], hz: f32, n: usize) -> Vec<f32> {
        seeded(algorithm, operators, hz, n, SEED)
    }

    /// [`voice`], under a stated seed — so the draw is what varies.
    fn seeded(
        algorithm: Algorithm,
        operators: [Operator; 4],
        hz: f32,
        n: usize,
        seed: u64,
    ) -> Vec<f32> {
        let levels = std::array::from_fn(|i| operators[i].level);
        let mut out = vec![0.0; n];
        let note = Note {
            gate: HELD,
            seed,
            rate: RATE,
        };
        render(&mut out, &vec![hz; n], algorithm, &operators, &levels, note);
        out
    }

    /// [`voice`], from stated start phases — what the two hand-computed tests
    /// below read, since a literal has to know where each operator began.
    fn from_phases(
        phase: [f32; FM_OPERATORS],
        algorithm: Algorithm,
        operators: [Operator; 4],
        hz: f32,
        n: usize,
    ) -> Vec<f32> {
        let levels = std::array::from_fn(|i| operators[i].level);
        let mut out = vec![0.0; n];
        let note = Note {
            gate: HELD,
            seed: SEED,
            rate: RATE,
        };
        render_from(
            phase,
            &mut out,
            &vec![hz; n],
            algorithm,
            &operators,
            &levels,
            note,
        );
        out
    }

    /// The amplitude of the sinusoid at `hz` in `buf`, by a one-bin DFT.
    fn level_at(buf: &[f32], hz: f32) -> f32 {
        let (mut re, mut im) = (0.0f64, 0.0f64);
        for (i, s) in buf.iter().enumerate() {
            let phase = std::f64::consts::TAU * f64::from(hz) * i as f64 / f64::from(RATE);
            re += f64::from(*s) * phase.cos();
            im -= f64::from(*s) * phase.sin();
        }
        (2.0 * re.hypot(im) / buf.len() as f64) as f32
    }

    /// The two things a magnitude spectrum cannot see: that a modulator is
    /// **added** into the carrier's phase, and that a phase walks **forward**.
    ///
    /// One carrier at ratio 1 bent by one modulator at ratio 1 and index 2,
    /// both **stated** to start at zero, so the buffer is `sin(x + 2 sin x)`
    /// with `x = 2π × 100 t` — and the normalisation is 1 because the lone
    /// carrier is the only one at any level. The literals below are worked out
    /// from that formula by hand; recomputing the renderer's expression here
    /// would only assert that the code agrees with itself.
    ///
    /// - **Sample 0** is `sin(0)`, which every variant agrees on — it is here
    ///   to state that the pass starts where it was *told* to, which is the
    ///   half of the claim that survived the operators no longer starting at
    ///   zero on their own.
    /// - **Sample 40** is `x = 0.5699`, where the three readings are furthest
    ///   apart: `sin(0.5699 + 1.0795) = 0.9969` forward, `sin(0.5699 − 1.0795)
    ///   = −0.4875` if the modulator were subtracted, and `−0.9969` if the
    ///   phase ran backwards. No two of those are within a tolerance of each
    ///   other.
    #[test]
    fn the_carrier_is_added_in_and_walks_forward() {
        let buf = from_phases(
            [0.0; FM_OPERATORS],
            Algorithm::Twin,
            [op(1.0, 2.0), op(1.0, 1.0), op(1.0, 0.0), op(1.0, 0.0)],
            BASE,
            512,
        );
        assert_eq!(buf[0], 0.0, "both phases were stated at zero, so sin(0)");
        assert!(
            (buf[40] - 0.996_94).abs() < 1e-4,
            "sample 40 read {}, where subtracting reads −0.4875 and a \
             backwards phase −0.9969",
            buf[40]
        );
    }

    /// A note rendered with the feedback path driven as hard as a document can
    /// ask for still stays inside unity and still makes a sound — the bound
    /// [`feedback`](super::super::feedback) proves arithmetically, seen through
    /// a whole buffer.
    #[test]
    fn feedback_is_bounded_however_it_is_written() {
        let mut driven = op(1.0, 1.0);
        driven.feedback = 1e9;
        let buf = voice(Algorithm::Parallel, [driven; 4], BASE, WINDOW);
        assert!(buf.iter().all(|s| s.is_finite() && s.abs() <= 1.0));
        assert!(buf.iter().any(|s| s.abs() > 0.5), "and it is not silence");
    }

    /// The two bends an operator takes — from its modulators and from its own
    /// feedback — are **summed**, not differenced, and only a signed sample can
    /// say so. Every spectral assertion in [`routing`] reads a magnitude, and a
    /// magnitude is blind to the sign of one term inside a phase; the two
    /// tests above each hold one of the terms at zero, so neither notices
    /// either.
    ///
    /// One carrier at ratio 1 with feedback at full, under one modulator at
    /// ratio 1 and index 2, both stated to start at zero, played at an eighth
    /// of the sample rate so a sample is an eighth of a cycle and the recursion
    /// is short enough to work by hand. Sample 0 is `sin(0)`; sample 1 has
    /// nothing fed back yet, so it is `sin(π/4 + 2 sin(π/4)) = 0.8087` either
    /// way. **Sample 2 is where they part**: the fed-back average is
    /// `π × 0.8087 / 2 = 1.2700`, so the forward sum reads
    /// `sin(π/2 + 2 + 1.2700) = −0.9917` and a difference would read
    /// `sin(π/2 + 2 − 1.2700) = +0.7454`. Opposite signs, and more than 1.7
    /// apart.
    #[test]
    fn the_modulators_and_the_feedback_add_rather_than_cancel() {
        let mut carrier = op(1.0, 1.0);
        carrier.feedback = 1.0;
        // An eighth of the sample rate: 5512.5 Hz, whose ratio ceiling is
        // exactly 4, so every operator here is comfortably inside it.
        let buf = from_phases(
            [0.0; FM_OPERATORS],
            Algorithm::Twin,
            [op(1.0, 2.0), carrier, op(1.0, 0.0), op(1.0, 0.0)],
            RATE / 8.0,
            8,
        );
        assert_eq!(buf[0], 0.0, "both phases were stated at zero");
        assert!(
            (buf[1] - 0.808_725).abs() < 1e-4,
            "sample 1 read {}",
            buf[1]
        );
        assert!(
            (buf[2] + 0.991_723).abs() < 1e-4,
            "sample 2 read {}, where a difference reads +0.7454",
            buf[2]
        );
    }

    /// The other end of that seam: what [`render`] hands the pass is the
    /// note's **own** draw, so a strike is reproducible under one seed, a
    /// different strike under another, and neither of them is the locked
    /// buffer this source used to produce.
    ///
    /// The last assertion is the one with work to do. A determinism test and a
    /// two-seeds-differ test would both pass on a renderer that had gone back
    /// to starting every operator at zero and folded the seed in somewhere
    /// else; only a comparison against the stated-zero render says the draw
    /// reaches the phases.
    #[test]
    fn a_note_starts_where_its_seed_says() {
        let operators = [op(1.0, 3.0), op(1.41, 2.0), op(2.0, 1.0), op(3.0, 1.0)];
        let strike = |seed| seeded(Algorithm::Fan, operators, BASE, WINDOW, seed);
        assert_eq!(strike(11), strike(11), "an fm4 note is not reproducible");
        assert_ne!(strike(11), strike(12), "two strikes are the same samples");
        let locked = from_phases([0.0; FM_OPERATORS], Algorithm::Fan, operators, BASE, WINDOW);
        assert_ne!(strike(11), locked, "the operators still start at zero");
    }

    /// And it is audible: a fed-back operator is no longer a sine, so harmonics
    /// appear that a clean one has none of.
    ///
    /// One sounding carrier under `parallel` rather than four copies of it —
    /// the claim is about an operator, and four of them at four drawn start
    /// phases sum their harmonics at four different angles, which is a fact
    /// about the summing and not about the feedback.
    #[test]
    fn feedback_turns_a_sine_into_something_with_harmonics() {
        let lone = |operator| [operator, op(1.0, 0.0), op(1.0, 0.0), op(1.0, 0.0)];
        let clean = voice(Algorithm::Parallel, lone(op(1.0, 1.0)), BASE, WINDOW);
        assert!(level_at(&clean, BASE * 2.0) < 1e-3, "a sine has no second");
        let mut rough = op(1.0, 1.0);
        rough.feedback = 0.9;
        let buf = voice(Algorithm::Parallel, lone(rough), BASE, WINDOW);
        assert!(level_at(&buf, BASE * 2.0) > 0.05, "the fed-back one does");
        assert!(level_at(&buf, BASE * 3.0) > 0.05);
    }

    /// The envelopes belong to the operators, not to the source: two carriers
    /// under one algorithm, one decaying and one held, and the spectrum says
    /// which is which as the note runs on.
    #[test]
    fn each_operator_follows_its_own_envelope() {
        let fading = Operator {
            env: Some(Adsr {
                a: 0.0,
                d: 0.25,
                s: 0.0,
                r: 0.0,
                curve: 0.0,
            }),
            ..op(1.0, 1.0)
        };
        let buf = voice(
            Algorithm::Twin,
            [op(1.0, 0.0), fading, op(1.0, 0.0), op(3.0, 1.0)],
            BASE,
            WINDOW * 4,
        );
        let (early, late) = (&buf[..WINDOW], &buf[WINDOW * 3..]);
        assert!(level_at(early, BASE) > 0.3, "the fading carrier starts up");
        assert!(level_at(late, BASE) < 0.01, "and has gone by the end");
        let held = (level_at(early, BASE * 3.0), level_at(late, BASE * 3.0));
        assert!(
            (held.0 - held.1).abs() < 0.02,
            "while the held carrier stays where it was: {held:?}"
        );
    }

    /// An operator past Nyquist is absent rather than folded. Operator 2 at
    /// ratio 30 on a 2 kHz note sits at 60 kHz and would alias to 15.9 kHz;
    /// nothing is there, and operator 1 has the whole mix because a dropped
    /// carrier leaves the normalisation too.
    #[test]
    fn an_operator_above_nyquist_is_dropped_rather_than_folded() {
        let high = 2000.0;
        let buf = voice(
            Algorithm::Parallel,
            [op(1.0, 1.0), op(30.0, 1.0), op(1.0, 0.0), op(1.0, 0.0)],
            high,
            WINDOW,
        );
        assert!(level_at(&buf, 15_900.0) < 1e-3, "no folded operator");
        assert!(
            (level_at(&buf, high) - 1.0).abs() < 1e-3,
            "and the one that sounds is not quieter for it"
        );
    }

    /// The routing decides which operators are dropped from the normalisation,
    /// so a modulator past Nyquist takes its bend away and leaves the level
    /// alone.
    #[test]
    fn a_modulator_above_nyquist_stops_bending_its_carrier() {
        let high = 2000.0;
        let buf = voice(
            Algorithm::Twin,
            [op(40.0, 6.0), op(1.0, 1.0), op(1.0, 0.0), op(1.0, 0.0)],
            high,
            WINDOW,
        );
        assert!(
            (level_at(&buf, high) - 1.0).abs() < 1e-3,
            "a bare carrier at full level"
        );
    }

    /// Whatever the routing and whatever the levels, the sum cannot leave
    /// `−1..=1`, and none of the eight is silent.
    #[test]
    fn every_algorithm_stays_inside_unity_and_makes_a_sound() {
        for algorithm in Algorithm::ALL {
            let buf = voice(
                algorithm,
                [op(1.0, 3.0), op(2.0, 4.0), op(3.0, 2.0), op(1.0, 1.0)],
                BASE,
                WINDOW,
            );
            let peak = buf.iter().fold(0.0f32, |m, s| m.max(s.abs()));
            assert!(peak <= 1.0, "{algorithm:?} peaked at {peak}");
            assert!(peak > 0.2, "{algorithm:?} is inaudible ({peak})");
        }
    }
}
