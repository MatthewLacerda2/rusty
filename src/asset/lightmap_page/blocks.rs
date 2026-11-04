//! Encoding a page's levels into blocks, and back (#1694), with
//! `block_compression`'s CPU encoders (a pure-Rust port of Intel's ISPC texture
//! compressor). Each level's bands of block rows encode on `threads` threads: the
//! blocks are the same whichever thread wrote them, so a bake's pages never
//! depend on the machine.

use block_compression::decode::{decompress_blocks_as_rgba16f, decompress_blocks_as_rgba8};
use block_compression::encode::{compress_rgba16, compress_rgba8};
use block_compression::half::f16;
use block_compression::{BC6HSettings, BC7Settings, CompressionVariant};
use glam::Vec3;

use super::{Page, PageFormat};

/// BC6H's profile. Lightmaps are smooth, so the very fast profile holds them within
/// 5% (`tests`); a 2048² page with its mips took 1.9 s on four cores of the 2026
/// cloud container, against 4.8 s at `basic`, which held them no closer.
const BC6H: CompressionVariant = CompressionVariant::BC6H(BC6HSettings::very_fast());
/// BC7's profile for a direction page (its alpha is the directionality): the one
/// the texture importer uses (`texture_import::encode`), 4.4 s a 2048² page there.
/// Not ultra-fast (1.4 s): like BC1 it flattens a block of two contrasting colours,
/// which a direction page has at every crease.
const BC7: CompressionVariant = CompressionVariant::BC7(BC7Settings::alpha_very_fast());

/// The largest value a BC6H unsigned-float texel holds (half float's).
pub const BC6H_MAX: f32 = 65504.0;

/// A BC6H page of linear HDR `levels` (level `l` is `(edge >> l)`² texels,
/// row-major). A negative, NaN or overflowing channel is clamped into BC6H's range.
pub fn encode_bc6h(edge: u32, levels: &[Vec<Vec3>], threads: usize) -> Page {
    let encoded = levels.iter().enumerate().map(|(l, texels)| {
        let halves: Vec<f16> = texels
            .iter()
            .flat_map(|c| {
                let [r, g, b] = c.to_array().map(|v| f16::from_f32(clean(v)));
                [r, g, b, f16::ONE]
            })
            .collect();
        banded(
            edge >> l,
            PageFormat::Bc6h,
            threads,
            |out, w, rows, row0| {
                let start = (row0 * w * 4) as usize;
                compress_rgba16(BC6H, &halves[start..], out, w, rows, w * 4);
            },
        )
    });
    Page {
        format: PageFormat::Bc6h,
        edge,
        levels: encoded.collect(),
    }
}

/// A BC7 page of RGBA8 `levels` (level `l` is `(edge >> l)`² texels, row-major).
pub fn encode_bc7(edge: u32, levels: &[Vec<[u8; 4]>], threads: usize) -> Page {
    let encoded = levels.iter().enumerate().map(|(l, texels)| {
        let bytes: &[u8] = bytemuck_flat(texels);
        banded(edge >> l, PageFormat::Bc7, threads, |out, w, rows, row0| {
            let start = (row0 * w * 4) as usize;
            compress_rgba8(BC7, &bytes[start..], out, w, rows, w * 4);
        })
    });
    Page {
        format: PageFormat::Bc7,
        edge,
        levels: encoded.collect(),
    }
}

/// Level `level` of a BC6H `page` decoded back to linear HDR: what a GPU samples.
///
/// The crate's decoders (0.11.0) lay BC6H out wrong: its `f32` one runs off the end
/// of its RGBA destination, and its half-float one writes each block's rows as RGB
/// (three halves a texel) into an RGBA-pitched buffer. The half-float one is read
/// through that layout: texel `(x, y)` is at `y * 4w + (x / 4) * 16 + (x % 4) * 3`.
pub fn decode_bc6h(page: &Page, level: usize) -> Vec<Vec3> {
    let edge = page.edge >> level;
    let pitch = (edge * 4) as usize;
    let mut out = vec![f16::ZERO; pitch * edge as usize];
    decompress_blocks_as_rgba16f(BC6H, edge, edge, &page.levels[level], &mut out);
    let (edge, mut texels) = (edge as usize, Vec::with_capacity(pitch * pitch / 16));
    for y in 0..edge {
        for x in 0..edge {
            let at = y * pitch + (x / 4) * 16 + (x % 4) * 3;
            texels.push(Vec3::new(
                out[at].to_f32(),
                out[at + 1].to_f32(),
                out[at + 2].to_f32(),
            ));
        }
    }
    texels
}

/// Level `level` of a BC7 `page` decoded back to RGBA8.
pub fn decode_bc7(page: &Page, level: usize) -> Vec<[u8; 4]> {
    let edge = page.edge >> level;
    let mut out = vec![0u8; (edge * edge * 4) as usize];
    decompress_blocks_as_rgba8(BC7, edge, edge, &page.levels[level], &mut out);
    out.chunks_exact(4)
        .map(|t| [t[0], t[1], t[2], t[3]])
        .collect()
}

/// Linear HDR texels as `Rgba16Float` bytes (alpha 1): the upload of a page on a
/// GPU without BC compression.
pub fn to_rgba16f(texels: &[Vec3]) -> Vec<u8> {
    texels
        .iter()
        .flat_map(|c| {
            let [r, g, b] = c.to_array().map(|v| f16::from_f32(clean(v)));
            [r, g, b, f16::ONE]
        })
        .flat_map(|h| h.to_le_bytes())
        .collect()
}

/// A channel clamped into what BC6H unsigned float holds; NaN reads 0.
fn clean(v: f32) -> f32 {
    if v.is_nan() {
        0.0
    } else {
        v.clamp(0.0, BC6H_MAX)
    }
}

/// RGBA8 texels as their bytes.
fn bytemuck_flat(texels: &[[u8; 4]]) -> &[u8] {
    block_compression::bytemuck::cast_slice(texels)
}

/// One `edge`² level's blocks, its block rows split in bands over `threads`:
/// `encode(out, width, rows, first_row)` fills one band.
fn banded(
    edge: u32,
    format: PageFormat,
    threads: usize,
    encode: impl Fn(&mut [u8], u32, u32, u32) + Sync,
) -> Vec<u8> {
    let mut blocks = vec![0u8; format.level_len(edge)];
    let row_bytes = format.level_len(edge) / (edge / 4).max(1) as usize;
    let block_rows = (edge / 4) as usize;
    let band = block_rows.div_ceil(threads.max(1)).max(1);
    let encode = &encode;
    std::thread::scope(|scope| {
        for (i, out) in blocks.chunks_mut(band * row_bytes).enumerate() {
            let rows = (out.len() / row_bytes) as u32 * 4;
            let row0 = (i * band * 4) as u32;
            scope.spawn(move || encode(out, edge, rows, row0));
        }
    });
    blocks
}
