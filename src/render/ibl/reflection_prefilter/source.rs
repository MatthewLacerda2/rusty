//! What both GGX prefilters read and write (#1696): the captured cube's source mip
//! chain, the mip a sample reads from it (filtered importance sampling), and the
//! half-float texels a baked cube stores.
//!
//! **Filtered importance sampling** (Křivánek & Colbert 2008, as Filament's `cmgen`
//! and UE's prefilter do): a sample drawn where the GGX lobe is thin covers a small
//! solid angle and reads the sharp base; one drawn in a wide lobe covers a large one
//! and reads a box-filtered mip whose texels are about that size. A small bright
//! source then reaches every sample of a rough lobe already spread over the texels
//! around it, so the rough mips are a smooth glow instead of speckle from the few
//! taps that land on it.
//!
//! The chain is built here, on the CPU, and quantized to half floats once, so the
//! GPU prefilter (which uploads it) and the CPU reference (which reads it) see the
//! same values, texel for texel.

#[cfg(test)]
use std::f32::consts::PI;

use glam::Vec3;

use super::mip_count;
use crate::core::color::{f16_to_f32, f32_to_f16};
use crate::render::CubemapCapture;

/// Bytes per texel of a prefiltered cube before it is encoded, and of a baked cube
/// decoded on an adapter without BC: `Rgba16Float`.
pub(crate) const TEXEL_BYTES: usize = 8;

/// The capture's source mip chain: level 0 the capture itself, each later level a
/// 2×2 box average of the one above, down to 1×1, every value rounded to a half
/// float as the GPU texture stores it. `chain[level][face]`, row-major.
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

/// A face of edge `size` box-filtered to half its edge.
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

/// Every texel rounded to the half float it will be stored as.
fn quantize(face: &[Vec3]) -> Vec<Vec3> {
    let round = |c: f32| f16_to_f32(f32_to_f16(c));
    (face.iter())
        .map(|c| Vec3::new(round(c.x), round(c.y), round(c.z)))
        .collect()
}

/// A face's texels as `Rgba16Float` bytes, alpha 1: what the GPU uploads and the
/// KTX2 file stores.
pub(crate) fn encode_face(face: &[Vec3]) -> Vec<u8> {
    let mut out = Vec::with_capacity(face.len() * TEXEL_BYTES);
    for c in face {
        for v in [c.x, c.y, c.z, 1.0] {
            out.extend_from_slice(&f32_to_f16(v).to_le_bytes());
        }
    }
    out
}

/// A stored face's texels, decoded from its half floats: [`encode_face`] undone.
#[cfg(test)]
pub(crate) fn decode_face(bytes: &[u8]) -> Vec<Vec3> {
    let half = |b: &[u8]| f16_to_f32(u16::from_le_bytes([b[0], b[1]]));
    (bytes.chunks_exact(TEXEL_BYTES))
        .map(|t| Vec3::new(half(&t[..2]), half(&t[2..4]), half(&t[4..6])))
        .collect()
}

/// The (fractional) source mip a GGX sample reads: the level whose texels subtend
/// the sample's solid angle, `0.5·log2(Ωs / Ωp) + 1`, clamped to the chain. `ndoth`
/// is the sample's half-vector cosine, `a2` roughness⁴, `samples` the sample count
/// and `base` the capture's face size. Under the split-sum's N = V the sample's pdf
/// is `D(h) / 4`, so `Ωs = 4 / (samples · D)`; a base texel is `Ωp = 4π / (6·base²)`.
/// The `+ 1` is Křivánek & Colbert's bias, which trades a little blur for noise.
/// `probe_prefilter.wgsl`'s `sample_mip` is this, line for line; only the CPU
/// reference calls it here.
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
