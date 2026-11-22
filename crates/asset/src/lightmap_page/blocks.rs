use block_compression::decode::decompress_blocks_as_rgba16f;
use block_compression::encode::compress_rgba16;
use block_compression::half::f16;
use block_compression::{BC6HSettings, CompressionVariant};
use glam::Vec3;

use super::{Page, PageFormat};
use crate::texture_import::banded;

/// Very fast holds smooth lightmaps within 5% (`tests`).
const BC6H: CompressionVariant = CompressionVariant::BC6H(BC6HSettings::very_fast());
/// Half float's largest finite value.
pub const BC6H_MAX: f32 = 65504.0;

/// Level `l` is `(edge >> l)`² texels, row-major. `None` when `stop` fires.
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

/// A level under a block (a cube's 2×2 and 1×1 mips) is padded to one 4×4 block
/// by repeating its last row and column, as wgpu stores it. Panics when `edge` is
/// neither under 4 nor a multiple of 4. Bands of block rows encode alone: the blocks
/// never depend on `threads`.
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

pub fn decode_bc6h(page: &Page, level: usize) -> Vec<Vec3> {
    bc6h_texels(page.edge >> level, &page.levels[level])
}

/// `block_compression` 0.11.0 decodes BC6H wrong: the `f32` decoder overruns, and the
/// half-float one writes RGB into an RGBA-pitched buffer, read as such here.
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

fn clean(v: f32) -> f32 {
    if v.is_nan() {
        0.0
    } else {
        v.clamp(0.0, BC6H_MAX)
    }
}
