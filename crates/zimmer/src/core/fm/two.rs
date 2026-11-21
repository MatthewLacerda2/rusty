//! two-operator FM.

use std::f32::consts::TAU;

use crate::hash::unit2;

const PHASE_CHANNEL: u64 = 0x464d;

const CARRIER: i64 = 0;

const MODULATOR: i64 = 1;

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

fn start_phase(operator: i64, seed: u64) -> f32 {
    unit2(operator, 0, PHASE_CHANNEL, seed)
}

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
    #[test]
    fn one_seed_renders_one_note_every_time() {
        let note = |seed| seeded_note(1.41, 6.0, 0.4, 4410, seed);
        assert_eq!(note(11), note(11), "an FM note is not reproducible");
    }

    #[test]
    fn a_second_strike_is_not_the_first_one_over_again() {
        let note = |seed| seeded_note(1.41, 6.0, 0.4, 4410, seed);
        assert_ne!(note(11), note(12), "two strikes are the same samples");
    }

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
