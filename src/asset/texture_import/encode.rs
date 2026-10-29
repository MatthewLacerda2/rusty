//! Choosing a block format and encoding an image's mip chain into it (#1574),
//! with `block_compression`'s CPU encoders (a pure-Rust port of Intel's ISPC
//! texture compressor). The bands of block rows encode on every core: the blocks
//! are the same whichever thread wrote them.

use block_compression::{encode::compress_rgba8, BC7Settings, CompressionVariant};
use image::{DynamicImage, RgbaImage};

use super::shrink::{half, to_bytes, to_working};
use super::{Compression, TextureKind, TextureSettings};

/// A block-compressed GPU format: 4×4 texels a block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlockFormat {
    /// RGB, 8 bytes a block (half a byte a texel).
    Bc1,
    /// One channel, 8 bytes a block.
    Bc4,
    /// Two channels, 16 bytes a block.
    Bc5,
    /// RGBA, 16 bytes a block (a byte a texel).
    Bc7,
}

impl BlockFormat {
    pub const ALL: [Self; 4] = [Self::Bc1, Self::Bc4, Self::Bc5, Self::Bc7];

    /// The bytes of one 4×4 block.
    pub fn block_bytes(self) -> u32 {
        match self {
            Self::Bc1 | Self::Bc4 => 8,
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

/// `image` (already fitted) as `kind` compressed with `settings`, every level its
/// *Generate Mip Maps* asks for; `None` when a side is not a multiple of 4.
pub(super) fn encode(
    image: &DynamicImage,
    settings: &TextureSettings,
    kind: TextureKind,
) -> Option<Compressed> {
    let (width, height) = (image.width(), image.height());
    if width % 4 != 0 || height % 4 != 0 {
        return None;
    }
    let base = image.to_rgba8();
    let format = format_for(&base, settings.compression, kind);
    let variant = variant(format, settings.compression);
    let count = match settings.generate_mipmaps {
        true => full_chain_len(width, height),
        false => 1,
    };
    let mut levels = vec![compress(&base, variant, format)];
    let mut working = to_working(&base, kind);
    for _ in 1..count {
        working = half(&working);
        levels.push(compress(&to_bytes(&working, kind), variant, format));
    }
    Some(Compressed {
        format,
        width,
        height,
        levels,
    })
}

/// The format a `kind` image holding `texels` compresses to (the module doc's table).
pub(super) fn format_for(
    texels: &RgbaImage,
    compression: Compression,
    kind: TextureKind,
) -> BlockFormat {
    let grey = || texels.pixels().all(|p| p[0] == p[1] && p[1] == p[2]);
    let opaque = || texels.pixels().all(|p| p[3] == 255);
    match kind {
        TextureKind::Normal => BlockFormat::Bc5,
        TextureKind::Mask if grey() => BlockFormat::Bc4,
        _ if compression == Compression::Normal && opaque() => BlockFormat::Bc1,
        _ => BlockFormat::Bc7,
    }
}

/// The encoder for `format`: BC7 searches harder at `HighQuality`.
fn variant(format: BlockFormat, compression: Compression) -> CompressionVariant {
    let high = compression == Compression::HighQuality;
    match format {
        BlockFormat::Bc1 => CompressionVariant::BC1,
        BlockFormat::Bc4 => CompressionVariant::BC4,
        BlockFormat::Bc5 => CompressionVariant::BC5,
        BlockFormat::Bc7 if high => CompressionVariant::BC7(BC7Settings::alpha_basic()),
        BlockFormat::Bc7 => CompressionVariant::BC7(BC7Settings::alpha_fast()),
    }
}

/// `level`'s blocks, its edge texels repeated out to whole blocks.
fn compress(level: &RgbaImage, variant: CompressionVariant, format: BlockFormat) -> Vec<u8> {
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
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get());
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
