//! src/soundgen/core/noise.rs — the one source of randomness in a bake (#357).
//!
//! Every stochastic element in the synth — the `noise` source, the Karplus-Strong
//! excitation — draws from here, and here draws from the **seeded integer hash**
//! `procgen` established ([`crate::procgen::hash`]). No `rand`, no wall clock: the
//! sample at index `i` is a pure function of `(i, channel, seed)`, which is what
//! makes "same patch + note + seed ⇒ byte-identical WAV" true rather than hoped for.

use crate::procgen::hash::unit2;

/// One white-noise sample in `−1..1` for position `i` on `channel`, under `seed`.
/// `channel` keeps two consumers (say the excitation and a noise layer) from
/// drawing the identical sequence.
#[inline]
pub fn white(i: usize, channel: u64, seed: u64) -> f32 {
    unit2(i as i64, 0, channel, seed) * 2.0 - 1.0
}

/// Fill `out` with white noise — the `noise` source, unshaped (the filter and amp
/// envelope downstream are what turn it into a gunshot or a footstep).
pub fn fill(out: &mut [f32], seed: u64) {
    for (i, s) in out.iter_mut().enumerate() {
        *s = white(i, 0, seed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noise_is_bounded_and_centred() {
        let mut buf = vec![0.0; 8192];
        fill(&mut buf, 7);
        assert!(buf.iter().all(|s| (-1.0..=1.0).contains(s)));
        let mean: f32 = buf.iter().sum::<f32>() / buf.len() as f32;
        assert!(mean.abs() < 0.05, "mean {mean} is not centred on silence");
    }

    #[test]
    fn the_same_seed_replays_the_same_noise() {
        let (mut a, mut b) = (vec![0.0; 256], vec![0.0; 256]);
        fill(&mut a, 42);
        fill(&mut b, 42);
        assert_eq!(a, b);
    }

    #[test]
    fn a_different_seed_or_channel_decorrelates() {
        let (mut a, mut b) = (vec![0.0; 256], vec![0.0; 256]);
        fill(&mut a, 42);
        fill(&mut b, 43);
        assert_ne!(a, b);
        assert_ne!(white(9, 0, 1), white(9, 1, 1));
    }
}
