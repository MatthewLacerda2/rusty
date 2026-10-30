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

pub(crate) use crate::asset::texture_import::TextureKind;

/// The view format a level of `kind` is read and written through: the sRGB one
/// averages colour in linear light, the raw one averages every other kind's bytes.
pub(crate) fn view_format(kind: TextureKind) -> wgpu::TextureFormat {
    match kind.is_colour() {
        true => super::IMAGE_FORMAT,
        false => super::DATA_FORMAT,
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
    /// full chain unless its *Generate Mip Maps* is off (Unity's default is on),
    /// averaged raw when its *sRGB* is off (`TextureKind::reading`).
    pub(crate) fn for_map(import: &TextureSettings, kind: TextureKind) -> Self {
        Self {
            generate: import.generate_mipmaps,
            kind: kind.reading(import),
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

pub(crate) use crate::asset::texture_import::full_chain_len;

#[cfg(test)]
mod gpu_tests;
#[cfg(test)]
mod tests;
