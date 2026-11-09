//! A tiny seeded generator for the lightmap bake (#438): SplitMix64, seeded per texel
//! from `(seed, mesh, texel)`. Every texel draws from its own stream, so the result
//! is a pure function of the inputs however the texels are spread across threads.

/// SplitMix64 state. Not shared: one per texel. A progressive bake keeps each
/// texel's between its passes (#1970), so it is `Clone`.
#[derive(Clone, Debug)]
pub(in crate::scene::lighting) struct Rng(u64);

impl Rng {
    /// The stream for texel `texel` of receiving mesh `mesh` under `seed`.
    pub(in crate::scene::lighting) fn new(seed: u64, mesh: u64, texel: u64) -> Self {
        let mut rng = Rng(seed ^ 0x9e37_79b9_7f4a_7c15);
        rng.0 ^= mix(mesh.wrapping_mul(0xff51_afd7_ed55_8ccd));
        rng.0 ^= mix(texel.wrapping_mul(0xc4ce_b9fe_1a85_ec53));
        rng
    }

    /// The stream of texel `texel`'s validity probes (#1949): its own, apart from
    /// [`Self::new`]'s, so how many paths the texel traces never moves them.
    pub(in crate::scene::lighting) fn probes(seed: u64, mesh: u64, texel: u64) -> Self {
        Self::new(seed ^ 0x6a09_e667_f3bc_c909, mesh, texel)
    }

    /// The stream a progressive bake draws texel `texel`'s displayed direct light
    /// from (#1970): its own, so what is shown between passes never moves the
    /// stream the finished texel is drawn from.
    pub(in crate::scene::lighting) fn preview(seed: u64, mesh: u64, texel: u64) -> Self {
        Self::new(seed ^ 0xbb67_ae85_84ca_a73b, mesh, texel)
    }

    /// The next value, uniform in `[0, 1)`.
    pub(in crate::scene::lighting) fn next_f32(&mut self) -> f32 {
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
