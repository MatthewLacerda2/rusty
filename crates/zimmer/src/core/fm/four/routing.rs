//! how far an operator's own modulators bend it, per sample.
//!
//! One half of what an [`Algorithm`] decides, and the half that is asked
//! inside the pass: which earlier operators reach this one, and by how much.
//! [`voicing`](super::super::voicing) is the other half — which operators are
//! *heard* and what the carriers are divided by — and the two are apart
//! because that one is answered once per note and this one once per operator
//! per sample.
//!
//! The mask only ever names operators below the one being bent, which is the
//! whole of why the pass terminates: everything it reads was written earlier
//! in the same walk of 1, 2, 3, 4.

use crate::patch::FM_OPERATORS;

/// How far this operator's modulators bend its phase, in radians: each one's
/// level times its already-shaped output.
///
/// `mask` only ever names operators below this one, so every value it reads
/// out of `shaped` was written earlier in the same pass.
pub(crate) fn modulation(
    mask: u8,
    levels: &[f32; FM_OPERATORS],
    shaped: &[f32; FM_OPERATORS],
) -> f32 {
    (0..FM_OPERATORS)
        .filter(|op| mask & (1 << op) != 0)
        .map(|op| levels[op] * shaped[op])
        .sum()
}

/// The routings by what they put in the spectrum.
///
/// Read as levels out of a one-bin DFT, with one window and one fundamental
/// chosen so that every frequency asserted lands exactly on a bin: 100 Hz over
/// 4410 samples, a tenth of a second, in which partial `k` completes exactly
/// `10k` cycles. There is no leakage for a wrong number to hide in.
///
/// A magnitude is blind to a sign flip and to a phase running backwards, and
/// to where in its cycle an operator started. That is deliberate here — a
/// routing is a claim about *which* frequencies are present — and it is the
/// reason [`super`] keeps two tests that read hand-computed samples instead.
#[cfg(test)]
mod tests {
    use super::super::{Note, render};
    use crate::patch::{Algorithm, Operator};

    /// The played pitch every test below measures from.
    const BASE: f32 = 100.0;

    /// The rate everything renders at, as the DSP sees it.
    const RATE: f32 = 44_100.0;

    /// A tenth of a second: a whole number of cycles of [`BASE`] and of every
    /// harmonic of it.
    const WINDOW: usize = 4410;

    fn op(ratio: f32, level: f32) -> Operator {
        Operator {
            ratio,
            level,
            feedback: 0.0,
            env: None,
        }
    }

    /// `operators` rendered under `algorithm` at `hz`, over `n` samples, with
    /// each operator's level taken as written, held throughout, under one
    /// arbitrary seed — every assertion here is about which frequencies are
    /// present, and none of them can see where a note started.
    fn voice(algorithm: Algorithm, operators: [Operator; 4], hz: f32, n: usize) -> Vec<f32> {
        let levels = std::array::from_fn(|i| operators[i].level);
        let mut out = vec![0.0; n];
        render(
            &mut out,
            &vec![hz; n],
            algorithm,
            &operators,
            &levels,
            Note {
                gate: 10.0,
                seed: 7,
                rate: RATE,
            },
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

    /// How much of the buffer's energy sits above the fourth harmonic — a
    /// brightness measure that a level change alone cannot move, since it is a
    /// share rather than an amount.
    fn spread(buf: &[f32]) -> f32 {
        let high: f32 = (5..20).map(|k| level_at(buf, BASE * k as f32)).sum();
        let all: f32 = (1..20).map(|k| level_at(buf, BASE * k as f32)).sum();
        high / all
    }

    /// Which operators are heard, read out of the spectrum rather than off the
    /// table: two carriers at different ratios put energy at two frequencies,
    /// and a routing that hears only the last of them puts energy at one.
    ///
    /// Operator 3's level is zero in both, so operator 4 is an unmodulated sine
    /// either way and the only difference between the two spectra is whether
    /// operator 2 reached the output at all.
    #[test]
    fn a_carrier_is_heard_and_a_modulator_is_not() {
        let operators = [op(2.0, 0.0), op(1.0, 1.0), op(5.0, 0.0), op(3.0, 1.0)];
        let twin = voice(Algorithm::Twin, operators, BASE, WINDOW);
        assert!(
            (level_at(&twin, BASE) - 0.5).abs() < 1e-3,
            "operator 2 is a carrier under twin, at half the mix"
        );
        assert!((level_at(&twin, BASE * 3.0) - 0.5).abs() < 1e-3);
        let chain = voice(Algorithm::Chain, operators, BASE, WINDOW);
        assert!(
            level_at(&chain, BASE) < 1e-3,
            "under chain only operator 4 is heard"
        );
        assert!(
            (level_at(&chain, BASE * 3.0) - 1.0).abs() < 1e-3,
            "and it has the whole mix to itself"
        );
    }

    /// A sideband that only one routing produces. Operator 1 modulates all
    /// three carriers under `fan` and only operator 2 under `pair_and_two`, so
    /// with operator 3 at ratio 5 the two routings differ at 4×: a sideband
    /// under one, silence under the other.
    #[test]
    fn one_modulator_reaches_three_carriers_only_under_fan() {
        // Three ratios whose sideband combs never meet: operator 2 puts energy
        // at whole multiples of the pitch, operator 3 at halves and operator 4
        // at quarters. Two carriers bent by one modulator would otherwise land
        // sidebands on each other and could cancel, which is a fact about FM
        // rather than about the routing under test. The window is doubled so a
        // quarter-multiple still falls exactly on a bin.
        let operators = [op(1.0, 3.0), op(1.0, 1.0), op(5.5, 1.0), op(9.25, 1.0)];
        let fan = voice(Algorithm::Fan, operators, BASE, WINDOW * 2);
        assert!(
            level_at(&fan, BASE * 4.5) > 0.05,
            "operator 3 is bent, so 5.5× − 1× is in the spectrum"
        );
        assert!(level_at(&fan, BASE * 8.25) > 0.05, "and so is 9.25× − 1×");
        let pair = voice(Algorithm::PairAndTwo, operators, BASE, WINDOW * 2);
        assert!(
            level_at(&pair, BASE * 4.5) < 1e-3,
            "under pair_and_two operators 3 and 4 are plain sines"
        );
        assert!(level_at(&pair, BASE * 8.25) < 1e-3);
        assert!(
            (level_at(&pair, BASE * 5.5) - 1.0 / 3.0).abs() < 1e-3,
            "and operator 3 is at its own ratio, at a third of the mix"
        );
    }

    /// `branch` and `fork` are the same shape numbered differently, which is a
    /// claim about *where a recipe writes its numbers* — so one operator list
    /// has to sound different under the two. Operator 2 at level 6 is a
    /// dead-end modulator of operator 3 under `branch` (whose own level is
    /// zero, so nothing reaches the carrier) and a modulator of the carrier
    /// itself under `fork`.
    #[test]
    fn the_same_operators_route_differently_under_branch_and_fork() {
        // The carrier sits at 8× so that every sideband it could grow stays
        // above zero Hz: a sideband that folded around DC would land back on
        // one of its own siblings and could cancel it, which is a fact about
        // FM rather than about the routing under test.
        let operators = [op(3.0, 0.0), op(1.0, 3.0), op(7.0, 0.0), op(8.0, 1.0)];
        let branch = voice(Algorithm::Branch, operators, BASE, WINDOW);
        assert!(
            (level_at(&branch, BASE * 8.0) - 1.0).abs() < 1e-3,
            "under branch the carrier is a bare sine"
        );
        assert!(level_at(&branch, BASE * 7.0) < 1e-3, "with no sidebands");
        assert!(level_at(&branch, BASE * 9.0) < 1e-3);
        let fork = voice(Algorithm::Fork, operators, BASE, WINDOW);
        assert!(
            level_at(&fork, BASE * 7.0) > 0.2,
            "under fork operator 2 bends the carrier, so 8× − 1× is there"
        );
        assert!(level_at(&fork, BASE * 9.0) > 0.2, "and 8× + 1×");
    }

    /// Depth stacks through a chain: under `stack` operator 1 bends operator 3
    /// directly, and under `chain` it reaches the carrier only through operator
    /// 2 — whose level is zero here, so the carrier comes out clean.
    #[test]
    fn a_chain_carries_depth_only_through_the_operator_between() {
        let operators = [op(1.0, 5.0), op(1.0, 0.0), op(1.0, 5.0), op(1.0, 1.0)];
        let chain = voice(Algorithm::Chain, operators, BASE, WINDOW);
        let stack = voice(Algorithm::Stack, operators, BASE, WINDOW);
        assert!(
            spread(&stack) > spread(&chain) * 1.5,
            "stack {} should out-spread chain {}",
            spread(&stack),
            spread(&chain)
        );
    }

    /// A modulator that reaches nothing bends nothing: an empty mask sums to
    /// no radians however loud the operators under it are.
    #[test]
    fn an_operator_nothing_routes_into_is_bent_by_nothing() {
        let levels = [3.0, 4.0, 5.0, 6.0];
        let shaped = [0.5, -0.5, 1.0, -1.0];
        assert_eq!(super::modulation(0, &levels, &shaped), 0.0);
        // Bit `n` is operator `n + 1`, and each contributes its own product.
        assert_eq!(super::modulation(0b0001, &levels, &shaped), 1.5);
        assert_eq!(super::modulation(0b0011, &levels, &shaped), -0.5);
    }
}
