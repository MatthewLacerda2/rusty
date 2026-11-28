//! stored-format: the `<source>.bctex` compressed copies in the import cache (`cache`), and an extracted texture's `<glb>.bctex/<image>.bctex` (`location`)
//!
//! A normal map kept at 16 bits is never compressed: BC5's 8 bits, one step per 0.45°, would bake a
//! gentle slope as terraces.

pub(crate) mod background;
mod cache;
mod encode;
mod location;
mod orphans;
mod refresh;
mod shrink;
pub mod variance;

use std::fs::File;
use std::io::{BufRead, BufReader, Seek};
use std::path::Path;

use image::ImageReader;

pub(crate) use background::queue_extracted;
pub use background::{import_in_background, queue, queue_changed};
pub use cache::write_blank;
pub(crate) use encode::banded;
pub use encode::{blocks_len, full_chain_len, max_skip, BlockFormat, Compressed};
pub use location::{cache_file, cache_path_in, extracted_cache, CACHE_SUFFIX};
pub use refresh::{images, imports, pending, refresh, refresh_all, Refreshed};
pub use shrink::{fit, fitted_size};

use super::convert::{source_digest, Seen};
use super::sidecar::{Compression, TextureSettings, TextureType};

/// Bump when `encode`'s blocks change for one input: it re-makes every cached texture.
pub const ENCODER_VERSION: u32 = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TextureKind {
    Colour,
    Data,
    Normal,
    Mask,
}

impl TextureKind {
    pub fn name(self) -> &'static str {
        match self {
            Self::Colour => "colour",
            Self::Data => "data",
            Self::Normal => "normal",
            Self::Mask => "mask",
        }
    }

    pub fn is_colour(self) -> bool {
        self == Self::Colour
    }

    fn of_settings(settings: &TextureSettings) -> Self {
        match settings.texture_type == TextureType::Default && settings.srgb {
            true => Self::Colour,
            false => Self::Data,
        }
    }

    pub fn reading(self, settings: &TextureSettings) -> Self {
        match self.is_colour() && is_linear(settings) {
            true => Self::Data,
            false => self,
        }
    }
}

pub fn is_linear(settings: &TextureSettings) -> bool {
    settings.texture_type == TextureType::Default && !settings.srgb
}

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

/// BC4 fits only a mask, whose red `forward::maps::map_factor` reads. `render`'s
/// `compressed::reads_as` is this table by GPU format: change both.
pub fn fits(texture_type: TextureType, kind: TextureKind) -> bool {
    match texture_type {
        TextureType::Default => true,
        TextureType::NormalMap => kind == TextureKind::Normal,
        TextureType::SingleChannel => kind == TextureKind::Mask,
    }
}

pub struct Loaded {
    pub texels: Texels,
    pub source_size: (u32, u32),
    /// A 16-bit normal map source, whatever this read's slot.
    pub deep_normal: bool,
}

pub enum Texels {
    Blocks(Compressed),
    Image(image::DynamicImage),
}

pub fn imported_size(
    source: &Path,
    settings: &TextureSettings,
) -> Option<((u32, u32), (u32, u32))> {
    let (w, h) = open(source).ok()?.into_dimensions().ok()?;
    Some(((w, h), fitted_size(w, h, settings.max_size)))
}

pub fn load(
    source: &Path,
    settings: &TextureSettings,
    kind: TextureKind,
    compress: bool,
) -> Option<Loaded> {
    load_skipping(source, settings, kind, compress, 0)
}

pub fn load_skipping(
    source: &Path,
    settings: &TextureSettings,
    kind: TextureKind,
    compress: bool,
    skip: u32,
) -> Option<Loaded> {
    let cached = compress && format_of(settings).is_some();
    if let Some(blocks) = cached.then(|| current(source, settings, skip)).flatten() {
        let source_size = open(source).ok()?.into_dimensions().ok()?;
        shrink::warn_if_shrunk(source, source_size, settings.max_size);
        return Some(Loaded {
            texels: Texels::Blocks(blocks),
            source_size,
            // No cache is current for a 16-bit normal map: `refresh` writes none, and an older
            // one was stamped from other bytes or settings.
            deep_normal: false,
        });
    }
    let image = open(source).ok()?.decode().ok()?;
    let source_size = (image.width(), image.height());
    shrink::warn_if_shrunk(source, source_size, settings.max_size);
    let deep_normal = keeps_deep(settings, image.color());
    Some(Loaded {
        texels: Texels::Image(fit(image, settings.max_size, kind.reading(settings))),
        source_size,
        deep_normal,
    })
}

pub fn keeps_deep(settings: &TextureSettings, color: image::ColorType) -> bool {
    settings.texture_type == TextureType::NormalMap && is_deep(color)
}

pub fn is_deep_normal(source: &Path, settings: &TextureSettings) -> bool {
    use image::ImageDecoder;
    settings.texture_type == TextureType::NormalMap
        && open(source)
            .and_then(|r| r.into_decoder())
            .is_ok_and(|d| keeps_deep(settings, d.color_type()))
}

pub fn is_deep(color: image::ColorType) -> bool {
    color.bytes_per_pixel() > color.channel_count()
}

fn current(source: &Path, settings: &TextureSettings, skip: u32) -> Option<Compressed> {
    let Seen::Read(digest) = source_digest(source) else {
        return None;
    };
    cache::read(
        &cache_file(source).ok()?,
        cache::stamp(digest, settings),
        skip,
    )
}

/// By extension first, as `image::open` does: a TGA has no signature.
fn reader<R: BufRead + Seek>(source: &Path, read: R) -> image::ImageResult<ImageReader<R>> {
    match image::ImageFormat::from_path(source) {
        Ok(format) => Ok(ImageReader::with_format(read, format)),
        Err(_) => Ok(ImageReader::new(read).with_guessed_format()?),
    }
}

fn open(source: &Path) -> image::ImageResult<ImageReader<BufReader<File>>> {
    reader(source, BufReader::new(File::open(source)?))
}

#[cfg(test)]
mod cache_tests;
#[cfg(test)]
mod deep_tests;
#[cfg(test)]
mod extracted_tests;
#[cfg(test)]
mod load_tests;
#[cfg(test)]
mod orphans_tests;
#[cfg(test)]
mod shrink_tests;
#[cfg(test)]
mod skip_tests;
#[cfg(test)]
mod srgb_tests;
#[cfg(test)]
mod tests;
