//! src/soundgen/fx — the post-chain: delay, reverb, and the mandatory limiter
//! (#357).
//!
//! Three effects, deliberately. [`delay`] and [`reverb`] are the two that place a
//! dry synthesized sound *somewhere* — the difference between a gunshot recorded in
//! an anechoic chamber and one fired in a corridor — and the agent chooses them per
//! patch, applied in list order. [`limiter`] is not a choice: every bake passes
//! through it, because a clipped WAV is a broken asset, not a stylistic option.
//!
//! Distortion, chorus and the rest wait until a real sound cannot be made without
//! them (the layer's standing rule). Effects are added here, not in the patch
//! document's signal path, so the fixed source → filter → amp contract stays fixed.

pub mod delay;
pub mod limiter;
pub mod reverb;

use crate::soundgen::patch::Fx;

/// Longest fx tail a note is padded by, in seconds. A chain of long effects should
/// not turn a 200 ms blip into a half-minute file.
const MAX_TAIL: f32 = 6.0;

/// Apply the chain to `buf` in place, in list order.
pub fn apply_chain(buf: &mut [f32], chain: &[Fx], rate: f32) {
    for fx in chain {
        match *fx {
            Fx::Delay {
                time,
                feedback,
                mix,
            } => delay::apply(buf, time, feedback, mix, rate),
            Fx::Reverb { size, damp, mix } => reverb::apply(buf, size, damp, mix, rate),
        }
    }
}

/// How much extra time the chain needs to ring out after the note itself ends —
/// what the renderer pads the buffer by, so an echo or a reverb tail is never cut
/// off mid-repeat. A fully dry effect asks for nothing.
pub fn tail_seconds(chain: &[Fx]) -> f32 {
    let total: f32 = chain
        .iter()
        .map(|fx| match *fx {
            Fx::Delay {
                time,
                feedback,
                mix,
            } if mix > 0.0 => delay::tail_seconds(time, feedback),
            Fx::Reverb { size, mix, .. } if mix > 0.0 => reverb::tail_seconds(size),
            _ => 0.0,
        })
        .sum();
    total.min(MAX_TAIL)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn impulse(n: usize) -> Vec<f32> {
        let mut buf = vec![0.0; n];
        buf[0] = 1.0;
        buf
    }

    #[test]
    fn an_empty_chain_changes_nothing() {
        let mut buf = impulse(512);
        apply_chain(&mut buf, &[], 44_100.0);
        assert_eq!(buf, impulse(512));
        assert_eq!(tail_seconds(&[]), 0.0);
    }

    #[test]
    fn the_chain_applies_in_list_order() {
        // Delay-then-reverb smears the echoes; reverb-then-delay echoes the smear.
        // They must differ, which proves order is honoured rather than commuted.
        let chain = [
            Fx::Delay {
                time: 0.05,
                feedback: 0.4,
                mix: 0.5,
            },
            Fx::Reverb {
                size: 0.6,
                damp: 0.5,
                mix: 0.5,
            },
        ];
        let mut forward = impulse(22_050);
        let mut reversed = impulse(22_050);
        apply_chain(&mut forward, &chain, 44_100.0);
        apply_chain(&mut reversed, &[chain[1], chain[0]], 44_100.0);
        assert_ne!(forward, reversed);
        assert!(forward.iter().all(|s| s.is_finite()));
    }

    #[test]
    fn the_tail_covers_every_wet_effect_but_stays_bounded() {
        let wet = [
            Fx::Delay {
                time: 0.25,
                feedback: 0.5,
                mix: 0.5,
            },
            Fx::Reverb {
                size: 1.0,
                damp: 0.5,
                mix: 0.5,
            },
        ];
        assert!(tail_seconds(&wet[..1]) > 0.0);
        assert!(tail_seconds(&wet) > tail_seconds(&wet[..1]));
        assert!(tail_seconds(&wet) <= MAX_TAIL);
        let long = vec![
            Fx::Reverb {
                size: 1.0,
                damp: 0.5,
                mix: 1.0,
            };
            8
        ];
        assert_eq!(
            tail_seconds(&long),
            MAX_TAIL,
            "capped, however long the chain"
        );
    }

    #[test]
    fn a_dry_effect_asks_for_no_tail() {
        let dry = [
            Fx::Delay {
                time: 1.0,
                feedback: 0.9,
                mix: 0.0,
            },
            Fx::Reverb {
                size: 1.0,
                damp: 0.5,
                mix: 0.0,
            },
        ];
        assert_eq!(tail_seconds(&dry), 0.0);
    }
}
