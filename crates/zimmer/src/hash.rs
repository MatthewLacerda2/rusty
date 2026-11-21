//! dependency-free deterministic value hashing.
//!
//! Every stochastic op (white noise, noise jitter, Voronoi cell points) draws its
//! randomness from a pure integer hash of `(x, y, seed)` — never from `rand` (the
//! project has no `rand` crate, by design) and never from wall-clock. Because the
//! hash is a pure function of its inputs, the same recipe + seed always produces the
//! same buffer, which is what makes a bake byte-identical across runs.
//!
//! The mixer is the SplitMix64 / PCG-style integer finalizer: a handful of
//! xor-shift + odd-multiply rounds with good avalanche. It needs no state and no
//! crate, and over the integer-coordinate lattice the procedural ops sample it is
//! more than random enough for texture work.

/// 64-bit avalanche mixer (the SplitMix64 finalizer). Pure: identical input ⇒
/// identical output, the contract the determinism requirement rests on.
#[inline]
fn mix64(mut z: u64) -> u64 {
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

/// Hash a 2-D integer lattice cell `(x, y)` under `seed` to a `u32`. The two axes
/// and the seed are folded into one 64-bit word with distinct odd primes so that
/// `(x, y)` and `(y, x)` (and different seeds) decorrelate.
#[inline]
pub(crate) fn hash2(x: i64, y: i64, seed: u64) -> u32 {
    let h = seed
        .wrapping_mul(0x9e37_79b9_7f4a_7c15)
        .wrapping_add((x as u64).wrapping_mul(0xff51_afd7_ed55_8ccd))
        .wrapping_add((y as u64).wrapping_mul(0xc4ce_b9fe_1a85_ec53));
    (mix64(h) >> 32) as u32
}

/// As [`hash2`] but with an extra `channel` salt, so one lattice cell can yield
/// several independent values (e.g. a Voronoi cell's x and y jitter, or RGB white
/// noise) without them correlating.
#[inline]
pub(crate) fn hash3(x: i64, y: i64, channel: u64, seed: u64) -> u32 {
    hash2(x, y, seed ^ channel.wrapping_mul(0xd6e8_feb8_6659_fd93))
}

/// A `u32` hash mapped to `[0, 1)` as `f32`. The top 24 bits feed the f32 mantissa
/// so the result is uniform across representable values in the unit interval.
#[inline]
pub(crate) fn to_unit_f32(h: u32) -> f32 {
    (h >> 8) as f32 / (1u32 << 24) as f32
}

/// Convenience: a uniform `[0, 1)` sample for lattice cell `(x, y)` on `channel`.
#[inline]
pub(crate) fn unit2(x: i64, y: i64, channel: u64, seed: u64) -> f32 {
    to_unit_f32(hash3(x, y, channel, seed))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_is_pure_and_deterministic() {
        assert_eq!(hash2(3, 7, 42), hash2(3, 7, 42));
        assert_eq!(hash3(3, 7, 1, 42), hash3(3, 7, 1, 42));
    }

    #[test]
    fn hash_decorrelates_axes_seed_and_channel() {
        assert_ne!(hash2(3, 7, 42), hash2(7, 3, 42));
        assert_ne!(hash2(3, 7, 42), hash2(3, 7, 43));
        assert_ne!(hash3(3, 7, 0, 42), hash3(3, 7, 1, 42));
    }

    #[test]
    fn unit_sample_stays_in_unit_interval() {
        for x in 0..64 {
            for y in 0..64 {
                let u = unit2(x, y, 0, 7);
                assert!((0.0..1.0).contains(&u), "u={u} out of range");
            }
        }
    }

    #[test]
    fn unit_sample_is_roughly_uniform() {
        // Mean of a large uniform sample should sit near 0.5; a coarse sanity band.
        let n = 4096;
        let mut sum = 0.0f64;
        for i in 0..n {
            sum += unit2(i, i * 31 + 1, 0, 99) as f64;
        }
        let mean = sum / n as f64;
        assert!((0.4..0.6).contains(&mean), "mean={mean}");
    }
}
