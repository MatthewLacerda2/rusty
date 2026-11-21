//! four-operator FM: four sines, wired by the algorithm the recipe picked.

mod phase;
mod routing;

use std::f32::consts::TAU;

use super::{feedback, voicing};
use crate::core::{env, nyquist, NoteContext};
use crate::patch::{Algorithm, Operator, FM_OPERATORS};

pub(crate) fn render(
    out: &mut [f32],
    freqs: &[f32],
    algorithm: Algorithm,
    operators: &[Operator; FM_OPERATORS],
    levels: &[f32; FM_OPERATORS],
    note: NoteContext,
) {
    render_from(
        phase::starts(note.seed),
        out,
        freqs,
        (algorithm, operators, levels),
        note,
    );
}

fn render_from(
    mut phase: [f32; FM_OPERATORS],
    out: &mut [f32],
    freqs: &[f32],
    (algorithm, operators, levels): (Algorithm, &[Operator; FM_OPERATORS], &[f32; FM_OPERATORS]),
    note: NoteContext,
) {
    let NoteContext { gate, rate, .. } = note;
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

#[inline]
fn envelope(operator: &Operator, t: f32, gate: f32) -> f32 {
    match &operator.env {
        Some(adsr) => env::level_at(adsr, t, gate),
        None => 1.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::patch::Adsr;

    const BASE: f32 = 100.0;

    const RATE: f32 = 44_100.0;

    const WINDOW: usize = 4410;

    const HELD: f32 = 10.0;

    const SEED: u64 = 7;

    fn op(ratio: f32, level: f32) -> Operator {
        Operator {
            ratio,
            level,
            feedback: 0.0,
            env: None,
        }
    }

    fn voice(algorithm: Algorithm, operators: [Operator; 4], hz: f32, n: usize) -> Vec<f32> {
        seeded(algorithm, operators, hz, n, SEED)
    }

    fn seeded(
        algorithm: Algorithm,
        operators: [Operator; 4],
        hz: f32,
        n: usize,
        seed: u64,
    ) -> Vec<f32> {
        let levels = std::array::from_fn(|i| operators[i].level);
        let mut out = vec![0.0; n];
        let note = NoteContext {
            gate: HELD,
            seed,
            rate: RATE,
        };
        render(&mut out, &vec![hz; n], algorithm, &operators, &levels, note);
        out
    }

    fn from_phases(
        phase: [f32; FM_OPERATORS],
        algorithm: Algorithm,
        operators: [Operator; 4],
        hz: f32,
        n: usize,
    ) -> Vec<f32> {
        let levels = std::array::from_fn(|i| operators[i].level);
        let mut out = vec![0.0; n];
        let note = NoteContext {
            gate: HELD,
            seed: SEED,
            rate: RATE,
        };
        render_from(
            phase,
            &mut out,
            &vec![hz; n],
            (algorithm, &operators, &levels),
            note,
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

    #[test]
    fn feedback_is_bounded_however_it_is_written() {
        let mut driven = op(1.0, 1.0);
        driven.feedback = 1e9;
        let buf = voice(Algorithm::Parallel, [driven; 4], BASE, WINDOW);
        assert!(buf.iter().all(|s| s.is_finite() && s.abs() <= 1.0));
        assert!(buf.iter().any(|s| s.abs() > 0.5), "and it is not silence");
    }

    #[test]
    fn the_modulators_and_the_feedback_add_rather_than_cancel() {
        let mut carrier = op(1.0, 1.0);
        carrier.feedback = 1.0;
        // An eighth of the sample rate: 5512.5 Hz, whose ratio ceiling is exactly 4, so every
        // operator here is comfortably inside it.
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

    #[test]
    fn a_note_starts_where_its_seed_says() {
        let operators = [op(1.0, 3.0), op(1.41, 2.0), op(2.0, 1.0), op(3.0, 1.0)];
        let strike = |seed| seeded(Algorithm::Fan, operators, BASE, WINDOW, seed);
        assert_eq!(strike(11), strike(11), "an fm4 note is not reproducible");
        assert_ne!(strike(11), strike(12), "two strikes are the same samples");
        let locked = from_phases([0.0; FM_OPERATORS], Algorithm::Fan, operators, BASE, WINDOW);
        assert_ne!(strike(11), locked, "the operators still start at zero");
    }

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
