//! Encoding a page's levels into blocks, and back (#1694), with
//! `block_compression`'s CPU encoders (a pure-Rust port of Intel's ISPC texture
//! compressor). Each level's bands of block rows encode on `threads` threads: the
//! blocks are the same whichever thread wrote them, so a bake's pages never
//! depend on the machine.

use block_compression::decode::decompress_blocks_as_rgba16f;
use block_compression::encode::compress_rgba16;
use block_compression::half::f16;
use block_compression::{BC6HSettings, CompressionVariant};
use glam::Vec3;

use super::{Page, PageFormat};
use crate::texture_import::banded;

/// BC6H's profile. Lightmaps are smooth, so the very fast profile holds them within
/// 5% (`tests`); a 2048² page with its mips took 1.9 s on four cores of the 2026
/// cloud container, against 4.8 s at `basic`, which held them no closer.
const BC6H: CompressionVariant = CompressionVariant::BC6H(BC6HSettings::very_fast());
/// The largest value a BC6H unsigned-float texel holds (half float's).
pub const BC6H_MAX: f32 = 65504.0;

/// A BC6H page of linear HDR `levels` (level `l` is `(edge >> l)`² texels,
/// row-major). A negative, NaN or overflowing channel is clamped into BC6H's range.
/// `None` when `stop` answers true before a level.
pub fn encode_bc6h(
    edge: u32,
    levels: &[Vec<Vec3>],
    threads: usize,
    stop: &dyn Fn() -> bool,
) -> Option<Page> {
    let mut encoded = Vec::with_capacity(levels.len());
    for (l, texels) in levels.iter().enumerate() {
        if stop() {
            return None;
        }
        encoded.push(bc6h_level(edge >> l, texels, threads));
    }
    Some(Page {
        format: PageFormat::Bc6h,
        edge,
        levels: encoded,
    })
}

/// One `edge`² level of linear HDR `texels` (row-major) as BC6H unsigned-float
/// blocks, on `threads` threads: the one encoder lightmap pages and baked
/// reflection cubes (`render::ibl::ktx2_encode`, #1885) share. A level whose edge
/// is under a block (a cube's 2×2 and 1×1 mips) is padded to one 4×4 block by
/// repeating its last row and column, as wgpu stores such a mip; the padding is
/// never sampled. Channels are clamped as [`encode_bc6h`] says. Panics when `edge`
/// is neither under 4 nor a multiple of 4.
pub fn bc6h_level(edge: u32, texels: &[Vec3], threads: usize) -> Vec<u8> {
    let w = edge.max(4);
    assert!(
        w.is_multiple_of(4),
        "a {edge}-texel level is not whole blocks"
    );
    let e = edge.max(1) as usize;
    let at = |x: usize, y: usize| texels[y.min(e - 1) * e + x.min(e - 1)];
    let halves: Vec<f16> = (0..(w * w) as usize)
        .flat_map(|i| {
            let c = at(i % w as usize, i / w as usize);
            let [r, g, b] = c.to_array().map(|v| f16::from_f32(clean(v)));
            [r, g, b, f16::ONE]
        })
        .collect();
    banded(w, w, 16, threads, |out, rows, row0| {
        let start = (row0 * w * 4) as usize;
        compress_rgba16(BC6H, &halves[start..], out, w, rows, w * 4);
    })
}

/// Level `level` of a BC6H `page` decoded back to linear HDR: what a GPU samples.
pub fn decode_bc6h(page: &Page, level: usize) -> Vec<Vec3> {
    bc6h_texels(page.edge >> level, &page.levels[level])
}

/// The `edge`² texels a [`bc6h_level`] holds, decoded back to linear HDR, its
/// padding (a level under a block) dropped.
///
/// The crate's decoders (0.11.0) lay BC6H out wrong: its `f32` one runs off the end
/// of its RGBA destination, and its half-float one writes each block's rows as RGB
/// (three halves a texel) into an RGBA-pitched buffer. The half-float one is read
/// through that layout: texel `(x, y)` is at `y * 4w + (x / 4) * 16 + (x % 4) * 3`.
pub fn bc6h_texels(edge: u32, blocks: &[u8]) -> Vec<Vec3> {
    let w = edge.max(4);
    let pitch = (w * 4) as usize;
    let mut out = vec![f16::ZERO; pitch * w as usize];
    decompress_blocks_as_rgba16f(BC6H, w, w, blocks, &mut out);
    let edge = edge.max(1) as usize;
    let mut texels = Vec::with_capacity(edge * edge);
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
