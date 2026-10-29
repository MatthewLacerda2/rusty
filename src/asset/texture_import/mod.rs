//! src/asset/texture_import/ — an image made ready for the GPU at import (#1574):
//! fitted to its *Max Size* and block-compressed, mips included, the way Unity's
//! texture importer writes a platform format into its Library.
//!
//! **What it is compressed to** follows what the texels hold ([`TextureKind`],
//! the slot's first use) and the image's *Compression* ([`Compression`]):
//!
//! | kind | `Normal` | `HighQuality` |
//! |---|---|---|
//! | colour or data, opaque | BC1 | BC7 |
//! | colour or data, with alpha | BC7 | BC7 |
//! | normal map | BC5 (x, y; the shader rebuilds z) | BC5 |
//! | mask (metallic, roughness) holding one channel | BC4 | BC4 |
//!
//! A mask is single-channel when its red, green and blue agree on every texel (a
//! greyscale image); a packed one (glTF's occlusion-roughness-metallic) is data.
//! Colour keeps its sRGB decode: BC1 and BC7 have sRGB forms, and the renderer
//! picks them as it picks `Rgba8UnormSrgb` today.
//!
//! **Where it lives.** In the import cache beside converted models
//! (`convert::cache_location_as`), one file per source and kind, stamped with a
//! digest of the source's bytes, the settings and [`ENCODER_VERSION`]; any
//! difference re-encodes ([`cache`]). A game ships the cache, so it never
//! encodes unless a source changed.
//!
//! **When it is not compressed**: *Compression* `None`, or an image whose fitted
//! size is not a multiple of 4 on both sides (a BC texture's base level must be;
//! Unity leaves such a texture uncompressed too). The renderer then uploads RGBA8
//! as before, fitted by [`fit`].

mod cache;
mod encode;
mod shrink;

use std::path::Path;

pub use encode::{full_chain_len, BlockFormat, Compressed};
pub use shrink::fit;

use super::sidecar::{Compression, TextureSettings};

/// Bumped whenever [`encode`] would write different blocks for the same input,
/// so every cached texture is re-made.
pub const ENCODER_VERSION: u32 = 1;

/// What a texture's texels hold, from the slot that first uses it. It decides
/// how its mips average (colour in linear light, the rest raw) and what it is
/// compressed to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TextureKind {
    /// Authored in sRGB and sampled through the decoding view (albedo, emissive,
    /// UI images, particles).
    Colour,
    /// Raw values sampled as stored (shader texture slots, decal data maps).
    Data,
    /// A tangent-space normal map: only x and y are kept when compressed.
    Normal,
    /// A metallic or roughness map: one channel when its image is greyscale.
    Mask,
}

impl TextureKind {
    /// How a warning names the kind.
    pub fn name(self) -> &'static str {
        match self {
            Self::Colour => "colour",
            Self::Data => "data",
            Self::Normal => "normal",
            Self::Mask => "mask",
        }
    }

    /// Whether its texels are sRGB colour, averaged and sampled in linear light.
    pub fn is_colour(self) -> bool {
        self == Self::Colour
    }
}

/// The image at `source`, fitted and block-compressed for a slot holding `kind`
/// with import settings `settings`: read from the cache when it is current, else
/// encoded and cached. `None` when it is not to be compressed (see the module
/// doc) or cannot be read; the caller uploads it uncompressed.
pub fn compressed(
    source: &Path,
    settings: &TextureSettings,
    kind: TextureKind,
) -> Option<Compressed> {
    if settings.compression == Compression::None {
        return None;
    }
    let bytes = std::fs::read(source).ok()?;
    let stamp = cache::stamp(&bytes, settings, kind);
    let out = super::convert::cache_location_as(source, &format!(".{}.bctex", kind.name()));
    let out = out.ok()?;
    if let Some(hit) = cache::read(&out, stamp) {
        return Some(hit);
    }
    let image = image::load_from_memory(&bytes).ok()?;
    let made = encode::encode(&fit(image, settings.max_size, kind), settings, kind)?;
    if let Err(e) = cache::write(&out, stamp, &made) {
        log::warn!(
            "[Import] could not cache {}'s compressed texture: {e}",
            source.display()
        );
    }
    Some(made)
}

#[cfg(test)]
mod cache_tests;
#[cfg(test)]
mod tests;
