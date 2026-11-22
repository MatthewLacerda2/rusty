//! SplitMix64 seeded per texel from `(seed, mesh, texel)`: each texel draws from its own stream, so
//! the result is a pure function of the inputs however texels are spread across threads.

/// Not shared: one per texel; a progressive bake keeps each across passes, so it is `Clone`.
#[derive(Clone, Debug)]
pub(crate) struct Rng(u64);

impl Rng {
    pub(crate) fn new(seed: u64, mesh: u64, texel: u64) -> Self {
        let mut rng = Rng(seed ^ 0x9e37_79b9_7f4a_7c15);
        rng.0 ^= mix(mesh.wrapping_mul(0xff51_afd7_ed55_8ccd));
        rng.0 ^= mix(texel.wrapping_mul(0xc4ce_b9fe_1a85_ec53));
        rng
    }

    /// Apart from [`Self::new`]'s, so how many paths the texel traces never moves the validity
    /// probes.
    pub(crate) fn probes(seed: u64, mesh: u64, texel: u64) -> Self {
        Self::new(seed ^ 0x6a09_e667_f3bc_c909, mesh, texel)
    }

    /// Apart from the finished texel's stream, so what is shown between passes never moves it.
    pub(crate) fn preview(seed: u64, mesh: u64, texel: u64) -> Self {
        Self::new(seed ^ 0xbb67_ae85_84ca_a73b, mesh, texel)
    }

    pub(crate) fn next_f32(&mut self) -> f32 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        (mix(self.0) >> 40) as f32 / (1u64 << 24) as f32
    }
}

fn mix(mut z: u64) -> u64 {
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    z ^ (z >> 31)
}
