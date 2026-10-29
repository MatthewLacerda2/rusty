//! Choosing a block format and encoding an image's mip chain into it (#1574),
//! with `block_compression`'s CPU encoders (a pure-Rust port of Intel's ISPC
//! texture compressor). No BC1: its encoder (and `texpresso`'s) collapses a block
//! of two colours whose channels move against each other (red beside blue) to
//! their average, one flat grey; BC7 keeps both. The bands of block rows encode on
//! the [`Cores`] the caller gives: the blocks are the same whichever thread wrote
//! them.

use block_compression::{encode::compress_rgba8, BC7Settings, CompressionVariant};
use image::{DynamicImage, RgbaImage};

use super::shrink::{half, to_bytes, to_working};
use super::{Compression, TextureKind, TextureSettings};

/// A block-compressed GPU format: 4×4 texels a block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlockFormat {
    /// One channel, 8 bytes a block.
    Bc4,
    /// Two channels, 16 bytes a block.
    Bc5,
    /// RGBA, 16 bytes a block (a byte a texel).
    Bc7,
}

impl BlockFormat {
    pub const ALL: [Self; 3] = [Self::Bc4, Self::Bc5, Self::Bc7];

    /// The bytes of one 4×4 block.
    pub fn block_bytes(self) -> u32 {
        match self {
            Self::Bc4 => 8,
            Self::Bc5 | Self::Bc7 => 16,
        }
    }

    /// The bytes a `width`×`height` level takes: whole blocks, so a level smaller
    /// than one still takes one.
    pub fn level_bytes(self, width: u32, height: u32) -> usize {
        let blocks = width.div_ceil(4) as usize * height.div_ceil(4) as usize;
        blocks * self.block_bytes() as usize
    }
}

/// A texture ready to upload: its format, the base level's size and every mip
/// level's blocks, largest first.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Compressed {
    pub format: BlockFormat,
    pub width: u32,
    pub height: u32,
    pub levels: Vec<Vec<u8>>,
}

/// How many levels a full chain of a `width`×`height` image has, down to 1×1.
pub fn full_chain_len(width: u32, height: u32) -> u32 {
    32 - width.max(height).max(1).leading_zeros()
}

/// `image` (already fitted) compressed with `settings`, every level its
/// *Generate Mip Maps* asks for; `None` when it is left uncompressed (*Compression*
/// `None`, or a side that is not a multiple of 4).
pub(super) fn encode(
    image: &DynamicImage,
    settings: &TextureSettings,
    cores: Cores,
) -> Option<Compressed> {
    let (width, height) = (image.width(), image.height());
    if width % 4 != 0 || height % 4 != 0 {
        return None;
    }
    let format = super::format_of(settings)?;
    let kind = TextureKind::of_type(settings.texture_type);
    let base = image.to_rgba8();
    let variant = variant(format, settings.compression, &base);
    let count = match settings.generate_mipmaps {
        true => full_chain_len(width, height),
        false => 1,
    };
    let threads = cores.threads();
    let mut levels = vec![compress(&base, variant, format, threads)];
    let mut working = to_working(&base, kind);
    for _ in 1..count {
        working = half(&working);
        let level = to_bytes(&working, kind);
        levels.push(compress(&level, variant, format, threads));
    }
    Some(Compressed {
        format,
        width,
        height,
        levels,
    })
}

/// The encoder for `format` on `texels`. BC7's profiles are chosen for import
/// time: on one core of the 2026 cloud container a 2048² map took 8 to 11 s at
/// `Normal` (very fast) and 21 s at `HighQuality` (fast), the bands spread over
/// every core. The ultra-fast profiles (2 s) are not used: like BC1 they flatten
/// a block of two contrasting colours to one. An opaque image encodes three
/// channels, faster and closer.
fn variant(
    format: BlockFormat,
    compression: Compression,
    texels: &RgbaImage,
) -> CompressionVariant {
    let high = compression == Compression::HighQuality;
    let opaque = texels.pixels().all(|p| p[3] == 255);
    let bc7 = match (high, opaque) {
        (false, true) => BC7Settings::opaque_very_fast(),
        (false, false) => BC7Settings::alpha_very_fast(),
        (true, true) => BC7Settings::opaque_fast(),
        (true, false) => BC7Settings::alpha_fast(),
    };
    match format {
        BlockFormat::Bc4 => CompressionVariant::BC4,
        BlockFormat::Bc5 => CompressionVariant::BC5,
        BlockFormat::Bc7 => CompressionVariant::BC7(bc7),
    }
}

/// How many cores an encode spreads over.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cores {
    /// Every one: what Play and an export wait for.
    All,
    /// Half, at least one: the editor's background worker, so the editor stays
    /// responsive while it encodes.
    Half,
}

impl Cores {
    fn threads(self) -> usize {
        let all = std::thread::available_parallelism().map_or(1, |n| n.get());
        match self {
            Self::All => all,
            Self::Half => (all / 2).max(1),
        }
    }
}

/// `level`'s blocks, its edge texels repeated out to whole blocks, its bands of
/// block rows spread over `threads`.
fn compress(
    level: &RgbaImage,
    variant: CompressionVariant,
    format: BlockFormat,
    threads: usize,
) -> Vec<u8> {
    let (w, h) = (
        level.width().div_ceil(4) * 4,
        level.height().div_ceil(4) * 4,
    );
    let padded = RgbaImage::from_fn(w, h, |x, y| {
        *level.get_pixel(x.min(level.width() - 1), y.min(level.height() - 1))
    });
    let mut blocks = vec![0; format.level_bytes(w, h)];
    let row_bytes = format.level_bytes(w, 4);
    let block_rows = (h / 4) as usize;
    let band = block_rows.div_ceil(threads).max(1);
    let texel_row = (w * 4) as usize;
    std::thread::scope(|scope| {
        for (i, out) in blocks.chunks_mut(band * row_bytes).enumerate() {
            let rows = (out.len() / row_bytes) as u32;
            let texels = &padded.as_raw()[i * band * 4 * texel_row..];
            scope.spawn(move || compress_rgba8(variant, texels, out, w, rows * 4, w * 4));
        }
    });
    blocks
}
