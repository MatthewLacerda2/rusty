//! src/render/ibl/reflection_prefilter.rs — GGX specular prefilter for reflection probes (#245).
//!
//! Turns a captured static cubemap (LDR RGBA8, mip0) into a prefiltered mip chain where
//! mip N stores the environment convolved for roughness `N / (mips-1)` — the standard
//! Karis split-sum specular pre-integration. At shading time the forward pass selects the
//! mip from a surface's roughness (`textureSampleLevel`), so a mirror reads the sharp base
//! level and a rough surface reads a progressively blurrier one.
//!
//! The convolution is GGX importance sampling: for each output texel we treat its world
//! direction as both the surface normal and the view (the split-sum assumption N = V = R),
//! draw a low-discrepancy set of half-vectors weighted toward the GGX lobe, reflect the
//! view across each to get a light direction, and average the source cubemap there
//! weighted by `N·L`. mip0 is left a faithful copy (roughness 0 = a mirror).
//!
//! The bake runs it on the GPU (`gpu`, #1602). The CPU form (`cpu`, test-only) is the
//! reference it is held to, texel for texel; this file keeps what both share: the
//! chain's type and shape, its sample count and the roughness ↔ mip mapping. No
//! wall-clock / RNG (the Hammersley sequence is deterministic), so it is clean under
//! the determinism guard like the capture and SH projection it stands beside.

#[cfg(test)]
pub(crate) mod cpu;
pub(crate) mod gpu;

/// Number of importance samples per output texel for the rough mips. A few hundred taps
/// gives a smooth lobe at the small cubemap sizes a probe uses; raising it trades bake
/// time for less sampling noise. The GPU prefilters, the probes' (`gpu`) and the sky
/// cube's (`ibl::sky_cube`), take the same count, so probe-lit and sky-lit rough metal blur alike.
pub(crate) const SAMPLE_COUNT: u32 = 128;

/// A prefiltered cubemap: a chain of mips, each six LDR RGBA8 faces. `mips[0]` is the
/// sharp base (roughness 0); each later mip halves the edge length and raises roughness.
/// This is exactly the layout the KTX2 encoder writes (largest level first).
pub struct PrefilteredCubemap {
    /// `mips[level][face]` is `mip_size(level)² * 4` bytes, row-major top-to-bottom, in
    /// `CubemapFace::ALL` order.
    pub mips: Vec<[Vec<u8>; 6]>,
    /// Base (mip 0) edge length in pixels.
    pub base_size: u32,
}

impl PrefilteredCubemap {
    /// Number of mip levels.
    pub fn level_count(&self) -> usize {
        self.mips.len()
    }

    /// Edge length of mip `level` (each level halves, floored at 1).
    pub fn level_size(&self, level: usize) -> u32 {
        (self.base_size >> level).max(1)
    }
}

/// The fractional mip level a `roughness` in [0, 1] selects from a `level_count`-deep
/// chain: `roughness * (level_count - 1)`, so roughness 0 reads the sharp base level and
/// roughness 1 the roughest. This MIRRORS the forward shader's `textureSampleLevel`
/// argument (`forward::lighting::probe_reflection`, past the probe's `mip_base` in the
/// array), so the headless mapping and the GPU agree — the unit-testable form of the
/// shader's mip selection (#245).
pub fn roughness_to_mip(roughness: f32, level_count: u32) -> f32 {
    roughness.clamp(0.0, 1.0) * (level_count.max(1) - 1) as f32
}

/// Validate a prefiltered cube's mip layout: every level's six faces must be
/// `level_size² * 4` bytes. The encoder relies on this; exposing it keeps the chain's
/// shape (and the `level_size`/`roughness_to_mip` accessors) a checkable invariant rather
/// than an implicit assumption.
pub fn validate_layout(cube: &PrefilteredCubemap) -> bool {
    (0..cube.level_count()).all(|level| {
        let size = cube.level_size(level) as usize;
        cube.mips[level].iter().all(|f| f.len() == size * size * 4)
    })
}

/// Mip count for a base edge length: a full chain down to 1×1 (`log2(size) + 1`), so the
/// roughest mip is a single texel — the fully-diffuse limit of the lobe. The sky cube's
/// chain (`ibl::sky_cube`) is this one at its own face size.
pub(crate) fn mip_count(base_size: u32) -> u32 {
    32 - base_size.max(1).leading_zeros()
}

#[cfg(test)]
#[path = "reflection_prefilter_tests.rs"]
mod reflection_prefilter_tests;
