//! src/soundgen/core/env.rs — linear-segment ADSR envelopes (#357).
//!
//! One envelope shape serves both stages that need one: the mandatory **amp**
//! envelope (which decides whether a patch is a pluck, a pad or a stab) and the
//! optional **filter** envelope (which sweeps the cutoff). Segments are straight
//! lines in v1 — exponential curves are a polish pass, not a gate.
//!
//! The envelope is driven by the **gate**: the note is held for `gate` seconds
//! (attack → decay → sustain), then released from wherever it had got to. Releasing
//! from the *current* level, not from sustain, is what makes a note shorter than its
//! attack still fade out instead of clicking.

use crate::soundgen::patch::Adsr;

/// The envelope level at time `t` (seconds) for a note held `gate` seconds.
/// Always in `0..=1`.
pub fn level_at(adsr: &Adsr, t: f32, gate: f32) -> f32 {
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
    (from * (1.0 - (t - gate) / adsr.r)).max(0.0)
}

/// The attack → decay → sustain level, i.e. the envelope while the note is held.
fn held_level(adsr: &Adsr, t: f32) -> f32 {
    if adsr.a > 0.0 && t < adsr.a {
        return t / adsr.a;
    }
    let sustain = adsr.s.clamp(0.0, 1.0);
    let since_attack = t - adsr.a.max(0.0);
    if adsr.d > 0.0 && since_attack < adsr.d {
        return 1.0 - (1.0 - sustain) * (since_attack / adsr.d);
    }
    sustain
}

/// Sample the envelope into a per-sample track `n` long at `sample_rate`, the form
/// the render pipeline multiplies by.
pub fn track(adsr: &Adsr, gate: f32, n: usize, sample_rate: f32) -> Vec<f32> {
    (0..n)
        .map(|i| level_at(adsr, i as f32 / sample_rate, gate))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn adsr(a: f32, d: f32, s: f32, r: f32) -> Adsr {
        Adsr { a, d, s, r }
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
        // Gate ends halfway up a 1s attack: release starts at 0.5, not at sustain.
        let e = adsr(1.0, 0.0, 1.0, 0.5);
        assert!((level_at(&e, 0.5, 0.5) - 0.5).abs() < 1e-6);
        assert!((level_at(&e, 0.75, 0.5) - 0.25).abs() < 1e-6);
        assert_eq!(level_at(&e, 1.0, 0.5), 0.0);
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
