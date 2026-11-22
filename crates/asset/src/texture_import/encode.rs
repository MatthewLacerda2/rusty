use block_compression::{encode::compress_rgba8, BC7Settings, CompressionVariant};
use image::{DynamicImage, RgbaImage};

use super::shrink::{half, to_bytes, to_working};
use super::variance::{self, VarianceTable};
use super::{Compression, TextureKind, TextureSettings};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlockFormat {
    Bc4,
    Bc5,
    Bc7,
}

impl BlockFormat {
    /// The order is the `.bctex` format byte (`cache`): a reorder bumps `ENCODER_VERSION`.
    pub const ALL: [Self; 3] = [Self::Bc4, Self::Bc5, Self::Bc7];

    pub fn block_bytes(self) -> u32 {
        match self {
            Self::Bc4 => 8,
            Self::Bc5 | Self::Bc7 => 16,
        }
    }

    pub fn level_bytes(self, width: u32, height: u32) -> usize {
        blocks_len(width, height, self.block_bytes())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Compressed {
    pub format: BlockFormat,
    pub width: u32,
    pub height: u32,
    /// Every mip level's blocks, largest first.
    pub levels: Vec<Vec<u8>>,
    pub variance: VarianceTable,
}

pub fn full_chain_len(width: u32, height: u32) -> u32 {
    32 - width.max(height).max(1).leading_zeros()
}

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
    let kind = TextureKind::of_settings(settings);
    let base = image.to_rgba8();
    let variant = variant(format, settings.compression, &base);
    let count = match settings.generate_mipmaps {
        true => full_chain_len(width, height),
        false => 1,
    };
    let threads = cores.threads();
    let mut levels = vec![compress(&base, variant, format, threads)];
    let mut working = to_working(&base, kind);
    // `half` filters each channel alone and BC5 keeps only x and y, which `as_read` leaves, so one
    // pyramid gives both the blocks and the spread.
    let bc5 = format == BlockFormat::Bc5;
    if bc5 {
        working = variance::as_read(&working);
    }
    let mut spread = Vec::new();
    for _ in 1..count {
        working = half(&working);
        if bc5 {
            spread.push(variance::level_mean(&working));
        }
        let level = to_bytes(&working, kind);
        levels.push(compress(&level, variant, format, threads));
    }
    Some(Compressed {
        format,
        width,
        height,
        levels,
        variance: variance::table(&spread),
    })
}

/// Not BC7's ultra-fast profiles, though quicker: like BC1 they flatten a block of two
/// contrasting colours (red beside blue) to one.
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cores {
    All,
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
    let texel_row = (w * 4) as usize;
    banded(w, h, format.block_bytes(), threads, |out, rows, row0| {
        let texels = &padded.as_raw()[row0 as usize * texel_row..];
        compress_rgba8(variant, texels, out, w, rows, w * 4);
    })
}

pub fn blocks_len(width: u32, height: u32, block_bytes: u32) -> usize {
    width.div_ceil(4) as usize * height.div_ceil(4) as usize * block_bytes as usize
}

/// `width` and `height` are multiples of 4. `encode(out, rows, first_row)` fills one band of
/// `rows` texel rows; the blocks are the same whichever thread wrote them.
pub(crate) fn banded(
    width: u32,
    height: u32,
    block_bytes: u32,
    threads: usize,
    encode: impl Fn(&mut [u8], u32, u32) + Sync,
) -> Vec<u8> {
    let mut blocks = vec![0u8; blocks_len(width, height, block_bytes)];
    let row_bytes = blocks_len(width, 4, block_bytes);
    let band = ((height / 4) as usize).div_ceil(threads.max(1)).max(1);
    let encode = &encode;
    std::thread::scope(|scope| {
        for (i, out) in blocks.chunks_mut(band * row_bytes).enumerate() {
            let rows = (out.len() / row_bytes) as u32 * 4;
            let row0 = (i * band * 4) as u32;
            scope.spawn(move || encode(out, rows, row0));
        }
    });
    blocks
}
