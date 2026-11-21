//! the additive series: a spectrum stated rather than carved.

use std::f32::consts::TAU;

use crate::core::nyquist;
use crate::hash::unit2;
use crate::patch::Partial;

const PHASE_CHANNEL: u64 = 0x4144;

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

fn sounding(partial: &Partial, ceiling: f32) -> Option<f32> {
    let ratio = partial.ratio * (partial.detune_cents / 1200.0).exp2();
    (ratio > 0.0 && ratio < ceiling).then_some(ratio)
}

#[inline]
fn decay_at(t: f32, decay: f32) -> f32 {
    if decay <= 0.0 {
        return 1.0;
    }
    (-t / decay).exp()
}

fn start_phase(index: usize, seed: u64) -> f32 {
    unit2(index as i64, 0, PHASE_CHANNEL, seed)
}

#[cfg(test)]
mod tests {
    use super::*;

    const BASE: f32 = 100.0;

    const RATE: f32 = 44_100.0;

    const WINDOW: usize = 4410;

    fn partial(ratio: f32, gain: f32) -> Partial {
        Partial {
            ratio,
            gain,
            detune_cents: 0.0,
            decay: 0.0,
        }
    }

    fn series(partials: &[Partial], n: usize) -> Vec<f32> {
        let mut out = vec![0.0; n];
        render(partials, &vec![BASE; n], 11, &mut out, RATE);
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
        assert!(level_at(&buf, BASE * 5.0) < 1e-3);
    }

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

    #[test]
    fn the_series_is_normalised_by_the_gain_that_sounds() {
        let quiet = series(&[partial(1.0, 1.0), partial(2.0, 0.5)], 512);
        let loud = series(&[partial(1.0, 20.0), partial(2.0, 10.0)], 512);
        assert_eq!(quiet, loud);
    }

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

    #[test]
    fn the_sum_stays_inside_unity() {
        let full: Vec<Partial> = (1..=16).map(|k| partial(k as f32, 1.0)).collect();
        let buf = series(&full, WINDOW);
        let peak = buf.iter().fold(0.0f32, |m, s| m.max(s.abs()));
        assert!(peak <= 1.0, "peaked at {peak}");
        assert!(peak > 0.2, "and is not silence ({peak})");
    }

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
        let fundamental = level_at(&out, high);
        assert!(
            (fundamental - 1.0).abs() < 1e-3,
            "the survivor read {fundamental}"
        );
    }

    #[test]
    fn a_partial_landing_exactly_on_nyquist_is_dropped() {
        let ceiling = nyquist::ratio_ceiling(&[BASE], RATE);
        assert_eq!(ceiling, 220.5, "half the rate over the played pitch");
        assert_eq!(sounding(&partial(ceiling, 1.0), ceiling), None);
        assert!(sounding(&partial(220.49, 1.0), ceiling).is_some());
        let buf = series(&[partial(ceiling, 1.0)], 8);
        assert!(buf.iter().all(|s| *s == 0.0), "got {buf:?}");
    }

    #[test]
    fn which_partials_are_legal_depends_on_the_note() {
        let ceiling = |hz: f32| nyquist::ratio_ceiling(&[hz], RATE);
        assert_eq!(ceiling(100.0), 220.5);
        assert_eq!(ceiling(2000.0), 11.025);
        assert!(sounding(&partial(16.0, 1.0), ceiling(100.0)).is_some());
        assert!(sounding(&partial(16.0, 1.0), ceiling(2000.0)).is_none());
    }

    #[test]
    fn a_partial_at_or_below_dc_is_not_rendered() {
        for ratio in [0.0, -1.0, f32::NAN] {
            assert!(sounding(&partial(ratio, 1.0), 220.5).is_none(), "{ratio}");
        }
        assert!(series(&[partial(0.0, 1.0)], 256).iter().all(|s| *s == 0.0));
    }

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
        let early = &buf[..441];
        let late = &buf[17_640..];
        assert!(
            level_at(early, BASE * 4.0) > 0.1,
            "the fourth starts audible"
        );
        assert!(level_at(late, BASE * 4.0) < 1e-3, "and is gone by the end");
        assert!(level_at(late, BASE) > 0.4, "while the fundamental rings on");
    }

    #[test]
    fn a_partial_without_a_decay_holds_its_full_weight() {
        for decay in [0.0, -1.0] {
            assert_eq!(decay_at(0.0, decay), 1.0);
            assert_eq!(decay_at(10.0, decay), 1.0);
        }
        assert_eq!(decay_at(0.0, 0.5), 1.0, "a decay starts at full");
        assert!((decay_at(0.5, 0.5) - 0.367_879_4).abs() < 1e-6);
        assert!((decay_at(1.0, 0.5) - 0.135_335_28).abs() < 1e-6);
    }

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

    const MAX: usize = crate::patch::MAX_PARTIALS;

    #[test]
    fn a_series_with_no_gain_left_is_silence_not_a_divide_by_zero() {
        let silent = [partial(1.0, 0.0), partial(2.0, -1.0)];
        let buf = series(&silent, 512);
        assert!(buf.iter().all(|s| *s == 0.0), "got {:?}", &buf[..8]);
    }
}
