#[cfg(test)]
use std::f32::consts::PI;

use glam::Vec3;

use super::mip_count;
use crate::CubemapCapture;
use rusty_core::color::{f16_to_f32, f32_to_f16};

/// Bytes: `Rgba16Float`, four halves.
pub(crate) const TEXEL_BYTES: usize = 8;

pub(crate) fn source_chain(capture: &CubemapCapture) -> Vec<[Vec<Vec3>; 6]> {
    let base = capture.resolution.max(1);
    let mut chain = vec![capture.faces.clone().map(|face| quantize(&face))];
    for level in 1..mip_count(base) {
        let above = (base >> (level - 1)).max(1) as usize;
        let prev = &chain[level as usize - 1];
        let next = std::array::from_fn(|f| quantize(&halve(&prev[f], above)));
        chain.push(next);
    }
    chain
}

fn halve(face: &[Vec3], size: usize) -> Vec<Vec3> {
    let half = (size / 2).max(1);
    let at = |x: usize, y: usize| face[y.min(size - 1) * size + x.min(size - 1)];
    (0..half * half)
        .map(|i| {
            let (x, y) = (2 * (i % half), 2 * (i / half));
            (at(x, y) + at(x + 1, y) + at(x, y + 1) + at(x + 1, y + 1)) * 0.25
        })
        .collect()
}

/// To half floats, as the GPU's texture stores them, so the CPU reference reads what the GPU reads.
fn quantize(face: &[Vec3]) -> Vec<Vec3> {
    let round = |c: f32| f16_to_f32(f32_to_f16(c));
    (face.iter())
        .map(|c| Vec3::new(round(c.x), round(c.y), round(c.z)))
        .collect()
}

pub(crate) fn encode_face(face: &[Vec3]) -> Vec<u8> {
    let mut out = Vec::with_capacity(face.len() * TEXEL_BYTES);
    for c in face {
        for v in [c.x, c.y, c.z, 1.0] {
            out.extend_from_slice(&f32_to_f16(v).to_le_bytes());
        }
    }
    out
}

#[cfg(test)]
pub(crate) fn decode_face(bytes: &[u8]) -> Vec<Vec3> {
    let half = |b: &[u8]| f16_to_f32(u16::from_le_bytes([b[0], b[1]]));
    (bytes.chunks_exact(TEXEL_BYTES))
        .map(|t| Vec3::new(half(&t[..2]), half(&t[2..4]), half(&t[4..6])))
        .collect()
}

/// Mirrors `sample_mip` in `engine/shaders/ibl/probe_prefilter.wgsl`: change both together.
/// Filtered importance sampling (Křivánek & Colbert, their +1 bias); `a2` is roughness⁴.
#[cfg(test)]
pub(crate) fn sample_mip(ndoth: f32, a2: f32, samples: u32, base: u32) -> f32 {
    let d = ndoth * ndoth * (a2 - 1.0) + 1.0;
    let ggx = a2 / (PI * d * d);
    let omega_s = 4.0 / (samples as f32 * ggx);
    let omega_p = 4.0 * PI / (6.0 * (base * base) as f32);
    let last = (mip_count(base) - 1) as f32;
    (0.5 * (omega_s / omega_p).log2() + 1.0).clamp(0.0, last)
}

#[cfg(test)]
#[path = "source_tests.rs"]
mod tests;
