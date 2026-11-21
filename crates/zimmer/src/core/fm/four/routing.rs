//! how far an operator's own modulators bend it, per sample.

use crate::patch::FM_OPERATORS;

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

#[cfg(test)]
mod tests {
    use super::super::render;
    use crate::core::NoteContext;
    use crate::patch::{Algorithm, Operator};

    const BASE: f32 = 100.0;

    const RATE: f32 = 44_100.0;

    const WINDOW: usize = 4410;

    fn op(ratio: f32, level: f32) -> Operator {
        Operator {
            ratio,
            level,
            feedback: 0.0,
            env: None,
        }
    }

    fn voice(algorithm: Algorithm, operators: [Operator; 4], hz: f32, n: usize) -> Vec<f32> {
        let levels = std::array::from_fn(|i| operators[i].level);
        let mut out = vec![0.0; n];
        render(
            &mut out,
            &vec![hz; n],
            algorithm,
            &operators,
            &levels,
            NoteContext {
                gate: 10.0,
                seed: 7,
                rate: RATE,
            },
        );
        out
    }

    fn level_at(buf: &[f32], hz: f32) -> f32 {
        let (mut re, mut im) = (0.0f64, 0.0f64);
        for (i, s) in buf.iter().enumerate() {
            let phase = std::f64::consts::TAU * f64::from(hz) * i as f64 / f64::from(RATE);
            re += f64::from(*s) * phase.cos();
            im -= f64::from(*s) * phase.sin();
        }
        (2.0 * re.hypot(im) / buf.len() as f64) as f32
    }

    fn spread(buf: &[f32]) -> f32 {
        let high: f32 = (5..20).map(|k| level_at(buf, BASE * k as f32)).sum();
        let all: f32 = (1..20).map(|k| level_at(buf, BASE * k as f32)).sum();
        high / all
    }

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

    #[test]
    fn one_modulator_reaches_three_carriers_only_under_fan() {
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

    #[test]
    fn the_same_operators_route_differently_under_branch_and_fork() {
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

    #[test]
    fn an_operator_nothing_routes_into_is_bent_by_nothing() {
        let levels = [3.0, 4.0, 5.0, 6.0];
        let shaped = [0.5, -0.5, 1.0, -1.0];
        assert_eq!(super::modulation(0, &levels, &shaped), 0.0);
        assert_eq!(super::modulation(0b0001, &levels, &shaped), 1.5);
        assert_eq!(super::modulation(0b0011, &levels, &shaped), -0.5);
    }
}
