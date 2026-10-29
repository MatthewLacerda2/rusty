//! src/asset/texture_import/ — an image made ready for the GPU at import (#1574):
//! fitted to its *Max Size* and block-compressed, mips included, the way Unity's
//! texture importer writes a platform format into its Library.
//!
//! **What it is compressed to** is an import setting, never the slot that draws
//! it, so it can be made before any draw: the image's *Texture Type*
//! ([`TextureType`], Unity's `TextureImporterType`).
//!
//! | Texture Type | format | mips average |
//! |---|---|---|
//! | `Default` | BC7 (a byte a texel) | in linear light |
//! | `NormalMap` | BC5 (x and y, a byte a texel; the shader rebuilds z) | raw |
//! | `SingleChannel` | BC4 (red, half a byte a texel) | raw |
//!
//! `HighQuality` searches BC7's modes harder than `Normal`; BC4 and BC5 encode
//! the same at both. A `Default` texture keeps its sRGB storage: the renderer
//! samples it decoded in a colour slot and raw in a data slot, as it does
//! `Rgba8UnormSrgb`. A texture drawn in a slot its type cannot serve (a BC5 normal
//! map as albedo would lose blue) is uploaded uncompressed for that use
//! ([`fits`]), so no channel is ever lost.
//!
//! **When it is made.** At import, through the import cache's own refresh, never
//! at a draw: [`refresh`] makes one image's cache current, [`refresh_all`] every
//! image under a folder (what Play's refresh, `convert::refresh_sources`, and an
//! export run before they go on), and [`queue`] hands images to a worker thread,
//! so the editor stays responsive while it encodes (an image added, its settings
//! changed, a project opened). A draw that finds no current cache uploads RGBA8 for
//! now ([`load`]); the worker's note (`core::fs::written`) swaps it for the
//! compressed copy once made.
//!
//! **Where it lives.** In the import cache beside converted models
//! (`convert::cache_location_as`), `<source>.bctex`, stamped with a digest of the
//! source's bytes, the settings and [`ENCODER_VERSION`]; any difference re-encodes
//! (`cache`). An export ships it (`dev::export`), so a shipped game never encodes.
//!
//! **When it is not compressed**: *Compression* `None`, or an image whose fitted
//! size is not a multiple of 4 on both sides (a BC texture's base level must be;
//! Unity leaves such a texture uncompressed too). A downscale to *Max Size* rounds
//! its sides to multiples of 4, so only an image already that small is left so.

mod cache;
mod encode;
mod refresh;
mod shrink;

use std::path::{Path, PathBuf};

pub use encode::{full_chain_len, BlockFormat, Compressed};
pub use refresh::{images, queue, queue_changed, refresh, refresh_all};
pub use shrink::fit;

use super::sidecar::{Compression, TextureSettings, TextureType};

/// Bumped whenever `encode` would write different blocks for the same input,
/// so every cached texture is re-made.
pub const ENCODER_VERSION: u32 = 2;

/// What a slot reads a texture as: how an uncompressed upload's mips average,
/// and which compressed formats it can sample ([`fits`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TextureKind {
    /// Authored in sRGB and sampled through the decoding view (albedo, emissive,
    /// UI images, particles).
    Colour,
    /// Raw values sampled as stored (shader texture slots, decal data maps).
    Data,
    /// A tangent-space normal map.
    Normal,
    /// A metallic or roughness map.
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

    /// How a compressed texture of `texture_type` averages its mips.
    fn of_type(texture_type: TextureType) -> Self {
        match texture_type {
            TextureType::Default => Self::Colour,
            TextureType::NormalMap | TextureType::SingleChannel => Self::Data,
        }
    }
}

/// The block format a texture with `settings` is compressed to, `None` when it is
/// left uncompressed (the module doc's table).
pub fn format_of(settings: &TextureSettings) -> Option<BlockFormat> {
    if settings.compression == Compression::None {
        return None;
    }
    Some(match settings.texture_type {
        TextureType::Default => BlockFormat::Bc7,
        TextureType::NormalMap => BlockFormat::Bc5,
        TextureType::SingleChannel => BlockFormat::Bc4,
    })
}

/// Whether a compressed texture of `texture_type` reads right in a slot holding
/// `kind`: BC7 everywhere; BC5 only as a normal map (it has no blue); BC4 only
/// as a mask, whose red channel the shader reads (`forward::maps::map_factor`).
pub fn fits(texture_type: TextureType, kind: TextureKind) -> bool {
    match texture_type {
        TextureType::Default => true,
        TextureType::NormalMap => kind == TextureKind::Normal,
        TextureType::SingleChannel => kind == TextureKind::Mask,
    }
}

/// What [`load`] read: the cached blocks, or the decoded image fitted to its
/// *Max Size*, with the source image's own size (UI geometry measures in it).
pub struct Loaded {
    pub texels: Texels,
    pub source_size: (u32, u32),
}

/// A texture's texels as uploaded.
pub enum Texels {
    /// Block-compressed, every level, from the import cache.
    Blocks(Compressed),
    /// Uncompressed, fitted, its mips still to be drawn.
    Image(image::DynamicImage),
}

/// The image at `source` for a slot holding `kind`: its current compressed cache
/// when `compress` (the caller checked the GPU and [`fits`]) and there is one,
/// else decoded and fitted for `kind`. Never encodes: a missing or stale cache
/// is made by [`refresh`]. One read of the file serves both. `None` when it
/// cannot be read.
pub fn load(
    source: &Path,
    settings: &TextureSettings,
    kind: TextureKind,
    compress: bool,
) -> Option<Loaded> {
    let bytes = std::fs::read(source).ok()?;
    let cached = compress && format_of(settings).is_some();
    if let Some(blocks) = cached.then(|| current(source, &bytes, settings)).flatten() {
        let reader = image::ImageReader::new(std::io::Cursor::new(&bytes));
        let source_size = reader.with_guessed_format().ok()?.into_dimensions().ok()?;
        return Some(Loaded {
            texels: Texels::Blocks(blocks),
            source_size,
        });
    }
    let image = image::load_from_memory(&bytes).ok()?;
    let source_size = (image.width(), image.height());
    Some(Loaded {
        texels: Texels::Image(fit(image, settings.max_size, kind)),
        source_size,
    })
}

/// The cache of the image at `source`, holding `bytes`, when it is current.
fn current(source: &Path, bytes: &[u8], settings: &TextureSettings) -> Option<Compressed> {
    cache::read(&cache_file(source).ok()?, cache::stamp(bytes, settings))
}

/// Where the import cache keeps the image at `source`'s compressed copy.
pub fn cache_file(source: &Path) -> Result<PathBuf, String> {
    super::convert::cache_location_as(source, CACHE_SUFFIX).map_err(|e| e.to_string())
}

/// What a compressed copy's name adds to its source's.
pub const CACHE_SUFFIX: &str = ".bctex";

#[cfg(test)]
mod cache_tests;
#[cfg(test)]
mod tests;
