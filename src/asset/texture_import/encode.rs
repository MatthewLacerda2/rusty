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
use super::variance::{self, VarianceTable};
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
        blocks_len(width, height, self.block_bytes())
    }
}

/// A texture ready to upload: its format, the base level's size, every mip
/// level's blocks, largest first, and for BC5 what its mips' normals lost
/// (`variance`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Compressed {
    pub format: BlockFormat,
    pub width: u32,
    pub height: u32,
    pub levels: Vec<Vec<u8>>,
    /// Each level's mean spread of normals (#1702): all zero unless BC5.
    pub variance: VarianceTable,
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
    // A BC5 map's normals as the shader reads them, z rebuilt (#1702).
    let mut unit = (format == BlockFormat::Bc5).then(|| variance::as_read(&working));
    let mut spread = Vec::new();
    for _ in 1..count {
        working = half(&working);
        if let Some(normals) = unit.as_mut() {
            *normals = half(normals);
            spread.push(variance::level_mean(normals));
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
    let texel_row = (w * 4) as usize;
    banded(w, h, format.block_bytes(), threads, |out, rows, row0| {
        let texels = &padded.as_raw()[row0 as usize * texel_row..];
        compress_rgba8(variant, texels, out, w, rows, w * 4);
    })
}

/// The bytes of a `width`×`height` level in 4×4 blocks of `block_bytes`: whole
/// blocks, so a level smaller than one still takes one.
pub(crate) fn blocks_len(width: u32, height: u32, block_bytes: u32) -> usize {
    width.div_ceil(4) as usize * height.div_ceil(4) as usize * block_bytes as usize
}

/// A `width`×`height` level's blocks (both multiples of 4), its block rows split
/// in bands over `threads`: `encode(out, rows, first_row)` fills one band, `rows`
/// texel rows from texel row `first_row`. The blocks are the same whichever
/// thread wrote them. The texture importer and the lightmap pages
/// (`asset::lightmap_page`) both encode through it.
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
