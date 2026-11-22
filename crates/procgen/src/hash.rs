#[inline]
fn mix64(mut z: u64) -> u64 {
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}

#[inline]
pub fn node_seed(recipe: u64, node: u32) -> u64 {
    recipe ^ mix64(u64::from(node).wrapping_mul(0xd1b5_4a32_d192_ed03))
}

/// A graph instance's node seed: `node` (the inner node's own `seed`) with
/// the instance's `seed` folded in. An instance seed of 0 returns `node` untouched,
/// so an instance bakes like its nodes pasted by hand; any other changes every
/// inner seed, a node written without one included.
#[inline]
pub fn instance_seed(node: u32, instance: u32) -> u32 {
    if instance == 0 {
        return node;
    }
    let word = (u64::from(instance) << 32) | u64::from(node);
    (mix64(word ^ 0xa076_1d64_78bd_642f) >> 32) as u32
}

#[inline]
fn hash2(x: i64, y: i64, seed: u64) -> u32 {
    let h = seed
        .wrapping_mul(0x9e37_79b9_7f4a_7c15)
        .wrapping_add((x as u64).wrapping_mul(0xff51_afd7_ed55_8ccd))
        .wrapping_add((y as u64).wrapping_mul(0xc4ce_b9fe_1a85_ec53));
    (mix64(h) >> 32) as u32
}

#[inline]
fn hash3(x: i64, y: i64, channel: u64, seed: u64) -> u32 {
    hash2(x, y, seed ^ channel.wrapping_mul(0xd6e8_feb8_6659_fd93))
}

#[inline]
fn to_unit_f32(h: u32) -> f32 {
    (h >> 8) as f32 / (1u32 << 24) as f32
}

#[inline]
pub fn unit2(x: i64, y: i64, channel: u64, seed: u64) -> f32 {
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
    fn node_seed_zero_is_the_recipe_seed_and_others_differ() {
        for r in [0, 42, u64::MAX] {
            assert_eq!(node_seed(r, 0), r);
        }
        assert_ne!(node_seed(42, 1), 42);
        assert_ne!(node_seed(42, 1), node_seed(42, 2));
        // Folded with XOR, not OR: a recipe seed with every bit set still moves.
        assert_ne!(node_seed(u64::MAX, 1), u64::MAX);
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
        let n = 4096;
        let mut sum = 0.0f64;
        for i in 0..n {
            sum += unit2(i, i * 31 + 1, 0, 99) as f64;
        }
        let mean = sum / n as f64;
        assert!((0.4..0.6).contains(&mean), "mean={mean}");
    }
}
