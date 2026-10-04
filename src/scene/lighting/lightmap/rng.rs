//! A tiny seeded generator for the lightmap bake (#438): SplitMix64, seeded per texel
//! from `(seed, mesh, texel)`. Every texel draws from its own stream, so the result
//! is a pure function of the inputs however the texels are spread across threads.

/// SplitMix64 state. Not shared: one per texel.
pub(super) struct Rng(u64);

impl Rng {
    /// The stream for texel `texel` of receiving mesh `mesh` under `seed`.
    pub(super) fn new(seed: u64, mesh: u64, texel: u64) -> Self {
        let mut rng = Rng(seed ^ 0x9e37_79b9_7f4a_7c15);
        rng.0 ^= mix(mesh.wrapping_mul(0xff51_afd7_ed55_8ccd));
        rng.0 ^= mix(texel.wrapping_mul(0xc4ce_b9fe_1a85_ec53));
        rng
    }

    /// The next value, uniform in `[0, 1)`.
    pub(super) fn next_f32(&mut self) -> f32 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        (mix(self.0) >> 40) as f32 / (1u64 << 24) as f32
    }
}

/// The SplitMix64 finalizer: a pure avalanche mix.
fn mix(mut z: u64) -> u64 {
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}
