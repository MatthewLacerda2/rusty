//! Mip chains for uploaded images (#1322). Every image texture gets a full chain,
//! so a tiled surface far from the camera samples a level its footprint fits
//! instead of aliasing the full-size image; Unity generates mipmaps by default too.
//!
//! The chain is drawn on the GPU at upload ([`MipGenerator`], `mips.wgsl`), each
//! level a box filter of the one above (2×2 on even sides, 3 overlap-weighted taps
//! on odd ones). What the texels hold decides how they
//! average: a colour map in linear light (averaging the sRGB bytes darkens every
//! mip of a high-contrast texture), a data map its raw values.
//!
//! [`MipSettings::for_map`] is the one place an upload decides whether to build a
//! chain and how, from the texture's import settings (#1323).

mod blit;

pub(crate) use blit::MipGenerator;

use crate::asset::sidecar::TextureSettings;

/// What a texture's texels hold, which decides how its mips average. A texture's
/// first use decides it: the cache keeps one upload per file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TextureKind {
    /// Authored in sRGB and sampled through the decoding view (albedo, emissive,
    /// UI images, particles).
    Colour,
    /// Raw values sampled as stored (metallic, roughness, normal, extra slots).
    Data,
}

impl TextureKind {
    /// How a warning names the kind.
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Colour => "colour",
            Self::Data => "data",
        }
    }

    /// The view format a level of this kind is read and written through: the
    /// sRGB one averages in linear light, the raw one averages the bytes.
    pub(crate) fn view_format(self) -> wgpu::TextureFormat {
        match self {
            Self::Colour => super::IMAGE_FORMAT,
            Self::Data => super::DATA_FORMAT,
        }
    }
}

/// How an upload builds its mips.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct MipSettings {
    /// A full chain when true, the base level alone when false.
    pub generate: bool,
    pub kind: TextureKind,
}

impl MipSettings {
    /// The settings for a map with import settings `import`, used as `kind`: a
    /// full chain unless its *Generate Mip Maps* is off (Unity's default is on).
    pub(crate) fn for_map(import: &TextureSettings, kind: TextureKind) -> Self {
        Self {
            generate: import.generate_mipmaps,
            kind,
        }
    }

    /// How many levels a `width`×`height` upload with these settings has.
    pub(crate) fn levels(self, width: u32, height: u32) -> u32 {
        match self.generate {
            true => full_chain_len(width, height),
            false => 1,
        }
    }
}

/// How many levels a full chain of a `width`×`height` image has, down to 1×1.
pub(crate) fn full_chain_len(width: u32, height: u32) -> u32 {
    32 - width.max(height).max(1).leading_zeros()
}

#[cfg(test)]
mod gpu_tests;
#[cfg(test)]
mod tests;
