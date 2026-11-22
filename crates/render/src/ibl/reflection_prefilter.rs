//! GGX split-sum prefilter (Karis): mip N holds roughness `N / (mips - 1)`, mip 0 a mirror
//! copy, all linear and unclamped.

#[cfg(test)]
pub(crate) mod cpu;
pub(crate) mod gpu;
pub(crate) mod source;

pub(crate) use source::TEXEL_BYTES;

/// Importance samples per output texel; the sky cube's prefilter takes the same count, so
/// rough metal blurs alike under both.
pub(crate) const SAMPLE_COUNT: u32 = 128;

pub struct PrefilteredCubemap {
    /// `[level][face]`: `level_size`² texels of `TEXEL_BYTES`, row-major, `CubemapFace::ALL`
    /// order, largest level first, as the KTX2 encoder writes.
    pub mips: Vec<[Vec<u8>; 6]>,
    pub base_size: u32,
}

impl PrefilteredCubemap {
    pub fn level_count(&self) -> usize {
        self.mips.len()
    }

    pub fn level_size(&self, level: usize) -> u32 {
        (self.base_size >> level).max(1)
    }
}

/// Mirrors `probe_reflection`'s `lod` in `engine/shaders/forward/reflection.wgsl`, past
/// `mip_base`: change both together.
pub fn roughness_to_mip(roughness: f32, level_count: u32) -> f32 {
    roughness.clamp(0.0, 1.0) * (level_count.max(1) - 1) as f32
}

pub fn validate_layout(cube: &PrefilteredCubemap) -> bool {
    (0..cube.level_count()).all(|level| {
        let size = cube.level_size(level) as usize;
        cube.mips[level]
            .iter()
            .all(|f| f.len() == size * size * TEXEL_BYTES)
    })
}

pub(crate) fn mip_count(base_size: u32) -> u32 {
    32 - base_size.max(1).leading_zeros()
}

#[cfg(test)]
#[path = "reflection_prefilter_tests.rs"]
mod reflection_prefilter_tests;

#[cfg(test)]
#[path = "reflection_prefilter/hdr_tests.rs"]
mod hdr_tests;
