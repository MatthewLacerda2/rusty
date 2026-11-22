//! ADSR envelopes, straight or curved.

use crate::patch::Adsr;

/// Steepest bend either way. A guard: a steep negative curve overflows `exp` into `NaN`.
const MAX_CURVE: f32 = 8.0;

/// Level in `0..=1` at `t` seconds into a note held `gate` seconds.
pub(crate) fn level_at(adsr: &Adsr, t: f32, gate: f32) -> f32 {
    if t <= 0.0 {
        return 0.0;
    }
    if t < gate {
        return held_level(adsr, t);
    }
    let from = held_level(adsr, gate);
    if adsr.r <= 0.0 {
        return 0.0;
    }
    (from * (1.0 - approach((t - gate) / adsr.r, adsr.curve))).max(0.0)
}

fn held_level(adsr: &Adsr, t: f32) -> f32 {
    if adsr.a > 0.0 && t < adsr.a {
        return approach(t / adsr.a, adsr.curve);
    }
    let sustain = adsr.s.clamp(0.0, 1.0);
    let since_attack = t - adsr.a.max(0.0);
    if adsr.d > 0.0 && since_attack < adsr.d {
        return 1.0 - (1.0 - sustain) * approach(since_attack / adsr.d, adsr.curve);
    }
    sustain
}

fn approach(p: f32, curve: f32) -> f32 {
    let p = p.clamp(0.0, 1.0);
    // The formula is 0/0 at zero; this branch keeps a straight segment's arithmetic exact.
    if curve == 0.0 {
        return p;
    }
    let k = curve.clamp(-MAX_CURVE, MAX_CURVE);
    (1.0 - (-k * p).exp()) / (1.0 - (-k).exp())
}

pub(crate) fn track(adsr: &Adsr, gate: f32, n: usize, sample_rate: f32) -> Vec<f32> {
    (0..n)
        .map(|i| level_at(adsr, i as f32 / sample_rate, gate))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn adsr(a: f32, d: f32, s: f32, r: f32) -> Adsr {
        Adsr {
            a,
            d,
            s,
            r,
            curve: 0.0,
        }
    }

    fn curved(adsr: Adsr, curve: f32) -> Adsr {
        Adsr { curve, ..adsr }
    }

    #[test]
    fn the_four_segments_hit_their_documented_levels() {
        let e = adsr(0.1, 0.2, 0.5, 0.4);
        assert_eq!(level_at(&e, 0.0, 1.0), 0.0, "starts silent");
        assert!((level_at(&e, 0.05, 1.0) - 0.5).abs() < 1e-6, "mid-attack");
        assert!(
            (level_at(&e, 0.1, 1.0) - 1.0).abs() < 1e-6,
            "peak at attack end"
        );
        assert!((level_at(&e, 0.2, 1.0) - 0.75).abs() < 1e-6, "mid-decay");
        assert!((level_at(&e, 0.5, 1.0) - 0.5).abs() < 1e-6, "sustain");
        assert!((level_at(&e, 1.2, 1.0) - 0.25).abs() < 1e-6, "mid-release");
        assert!(level_at(&e, 1.4, 1.0) < 1e-6, "silent again at release end");
        assert_eq!(level_at(&e, 9.9, 1.0), 0.0, "and stays silent");
    }

    #[test]
    fn a_note_released_mid_attack_fades_from_where_it_was() {
        let e = adsr(1.0, 0.0, 1.0, 0.5);
        assert!((level_at(&e, 0.5, 0.5) - 0.5).abs() < 1e-6);
        assert!((level_at(&e, 0.75, 0.5) - 0.25).abs() < 1e-6);
        assert_eq!(level_at(&e, 1.0, 0.5), 0.0);
    }

    #[test]
    fn the_attack_ends_at_its_own_end_rather_than_including_it() {
        let straight_to_sustain = adsr(0.01, 0.0, 0.5, 0.1);
        assert_eq!(level_at(&straight_to_sustain, 0.01, 1.0), 0.5);
        let with_a_decay = adsr(0.01, 0.2, 0.5, 0.1);
        assert!((level_at(&with_a_decay, 0.01, 1.0) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn the_sustain_is_exactly_the_number_the_document_asked_for() {
        // For 0.09 and 0.1, `1 - (1 - s)` is not `s`: the decay must hand over before its end.
        for s in [0.09, 0.1, 0.3, 0.7] {
            let e = adsr(0.0, 0.25, s, 0.1);
            assert_eq!(level_at(&e, 0.25, 1.0), s, "{s}, as the decay ends");
            assert_eq!(level_at(&e, 0.5, 1.0), s, "{s}, well past it");
        }
    }

    #[test]
    fn zero_length_segments_degrade_instead_of_dividing_by_zero() {
        let e = adsr(0.0, 0.0, 0.8, 0.0);
        assert!(
            (level_at(&e, 0.01, 1.0) - 0.8).abs() < 1e-6,
            "straight to sustain"
        );
        assert_eq!(level_at(&e, 1.0, 1.0), 0.0, "no release: cuts at the gate");
        for v in track(&e, 0.5, 64, 44_100.0) {
            assert!(v.is_finite(), "no NaN from a degenerate envelope");
        }
    }

    #[test]
    fn a_curve_of_zero_is_the_identity_to_the_bit() {
        for step in 0..=100 {
            let p = step as f32 / 100.0;
            assert_eq!(approach(p, 0.0), p, "progress {p}");
        }
    }

    #[test]
    fn every_curve_runs_the_whole_way_from_zero_to_one() {
        for curve in [-8.0, -3.0, -0.5, 0.0, 0.5, 3.0, 8.0] {
            assert_eq!(approach(0.0, curve), 0.0, "curve {curve} at the start");
            assert!(
                (approach(1.0, curve) - 1.0).abs() < 1e-6,
                "curve {curve} at the end"
            );
        }
    }

    #[test]
    fn a_positive_curve_is_ahead_of_the_line_the_whole_way() {
        for step in 1..100 {
            let p = step as f32 / 100.0;
            assert!(approach(p, 3.0) > p, "progress {p}");
            assert!(approach(p, 8.0) > approach(p, 3.0), "steeper, at {p}");
        }
    }

    #[test]
    fn a_negative_curve_mirrors_the_positive_one() {
        for step in 0..=20 {
            let p = step as f32 / 20.0;
            let mirrored = 1.0 - approach(1.0 - p, 3.0);
            assert!(
                (approach(p, -3.0) - mirrored).abs() < 1e-6,
                "progress {p}: {} vs {mirrored}",
                approach(p, -3.0)
            );
        }
    }

    #[test]
    fn a_curved_envelope_sheds_its_level_earlier_than_a_straight_one() {
        let line = adsr(0.0, 0.4, 0.5, 0.4);
        let bent = curved(line, 4.0);
        for t in [0.05, 0.1, 0.2, 0.3, 0.5, 0.6, 0.7] {
            assert!(level_at(&bent, t, 0.4) < level_at(&line, t, 0.4), "at {t}s");
        }
        assert_eq!(level_at(&bent, 0.0, 0.4), 0.0, "still starts silent");
        assert!(level_at(&bent, 0.8, 0.4) < 1e-6, "and still ends silent");
    }

    #[test]
    fn a_curved_release_still_starts_from_the_level_it_was_at() {
        let e = curved(adsr(1.0, 0.0, 1.0, 0.5), 3.0);
        let at_gate = level_at(&e, 0.5, 0.5);
        assert!(at_gate > 0.0 && at_gate < 1.0, "mid-attack, at {at_gate}");
        assert!(
            (level_at(&e, 0.500_01, 0.5) - at_gate).abs() < 1e-3,
            "no step across the gate"
        );
        assert_eq!(level_at(&e, 1.0, 0.5), 0.0, "silent at the release end");
    }

    #[test]
    fn an_absurd_curve_is_clamped_instead_of_going_non_finite() {
        for curve in [1e9, -1e9, f32::MAX, f32::MIN] {
            let e = curved(adsr(0.1, 0.2, 0.5, 0.2), curve);
            for v in track(&e, 0.4, 1_000, 1_000.0) {
                assert!((0.0..=1.0).contains(&v), "curve {curve} produced {v}");
            }
        }
    }

    #[test]
    fn track_is_the_sampled_form_of_the_same_curve() {
        let e = adsr(0.01, 0.01, 0.5, 0.01);
        let t = track(&e, 0.05, 100, 1000.0);
        assert_eq!(t.len(), 100);
        for (i, v) in t.iter().enumerate() {
            assert!((v - level_at(&e, i as f32 / 1000.0, 0.05)).abs() < 1e-6);
            assert!((0.0..=1.0).contains(v));
        }
    }
}
