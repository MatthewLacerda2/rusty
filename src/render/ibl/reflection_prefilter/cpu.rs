//! The CPU GGX prefilter (#245): the reference the GPU one (`super::gpu`, #1602) is
//! held to, texel for texel. Only tests run it; the bake prefilters on the GPU.

use std::f32::consts::PI;

use glam::{Vec2, Vec3};

use super::{mip_count, PrefilteredCubemap, SAMPLE_COUNT};
use crate::render::ibl::cube_sample::{face_uv, linear_to_srgb, sample_linear, FaceProjections};
use crate::render::{CubemapCapture, CubemapFace};

/// GGX-prefilter a captured cubemap into a roughness mip chain. mip0 copies the capture
/// (a mirror); each rougher mip is importance-sampled from mip0's faces.
pub(crate) fn prefilter(capture: &CubemapCapture) -> PrefilteredCubemap {
    let base = capture.resolution.max(1);
    let levels = mip_count(base);
    let proj = FaceProjections::new();
    let mut mips = Vec::with_capacity(levels as usize);

    // mip0: a faithful copy of the capture (roughness 0 = a perfect mirror).
    mips.push(capture.faces.clone());

    for level in 1..levels {
        let size = (base >> level).max(1);
        let roughness = level as f32 / (levels - 1) as f32;
        mips.push(filter_mip(&proj, capture, size, roughness));
    }

    PrefilteredCubemap {
        mips,
        base_size: base,
    }
}

/// Build one rough mip: every texel of every face GGX-convolved from the source faces.
fn filter_mip(
    proj: &FaceProjections,
    capture: &CubemapCapture,
    size: u32,
    roughness: f32,
) -> [Vec<u8>; 6] {
    let src = &capture.faces;
    let src_res = capture.resolution.max(1);
    CubemapFace::ALL.map(|face| {
        let mut out = vec![0u8; (size * size * 4) as usize];
        for py in 0..size {
            for px in 0..size {
                let (u, v) = face_uv(px, py, size);
                let n = proj.direction(face, u, v);
                let color = convolve(proj, src, src_res, n, roughness);
                write_texel(&mut out, px, py, size, color);
            }
        }
        out
    })
}

/// Pack a linear-RGB colour into an output texel (sRGB-encoded, opaque).
fn write_texel(out: &mut [u8], px: u32, py: u32, size: u32, color: Vec3) {
    let idx = ((py * size + px) * 4) as usize;
    out[idx] = linear_to_srgb(color.x);
    out[idx + 1] = linear_to_srgb(color.y);
    out[idx + 2] = linear_to_srgb(color.z);
    out[idx + 3] = 255;
}

/// GGX split-sum convolution at normal `n` for `roughness`: importance-sample half-vectors
/// around `n`, reflect the view (= n) across each, and average the source radiance weighted
/// by `N·L`. Returns linear RGB.
fn convolve(
    proj: &FaceProjections,
    faces: &[Vec<u8>; 6],
    res: u32,
    n: Vec3,
    roughness: f32,
) -> Vec3 {
    let (tangent, bitangent) = basis(n);
    let mut color = Vec3::ZERO;
    let mut weight = 0.0;
    for i in 0..SAMPLE_COUNT {
        let xi = hammersley(i, SAMPLE_COUNT);
        let h = importance_sample_ggx(xi, roughness, n, tangent, bitangent);
        let l = (2.0 * n.dot(h) * h - n).normalize_or_zero();
        let ndotl = n.dot(l).max(0.0);
        if ndotl > 0.0 {
            color += sample_linear(proj, faces, res, l) * ndotl;
            weight += ndotl;
        }
    }
    if weight > 0.0 {
        color / weight
    } else {
        sample_linear(proj, faces, res, n)
    }
}

/// An orthonormal tangent basis around `n`.
fn basis(n: Vec3) -> (Vec3, Vec3) {
    let up = if n.z.abs() < 0.999 { Vec3::Z } else { Vec3::X };
    let tangent = up.cross(n).normalize_or_zero();
    let bitangent = n.cross(tangent);
    (tangent, bitangent)
}

/// A GGX-distributed half-vector in world space for the low-discrepancy point `xi` (Karis).
fn importance_sample_ggx(
    xi: Vec2,
    roughness: f32,
    n: Vec3,
    tangent: Vec3,
    bitangent: Vec3,
) -> Vec3 {
    let a = roughness * roughness;
    let phi = 2.0 * PI * xi.x;
    let cos_theta = ((1.0 - xi.y) / (1.0 + (a * a - 1.0) * xi.y)).sqrt();
    let sin_theta = (1.0 - cos_theta * cos_theta).max(0.0).sqrt();
    let h_local = Vec3::new(sin_theta * phi.cos(), sin_theta * phi.sin(), cos_theta);
    (tangent * h_local.x + bitangent * h_local.y + n * h_local.z).normalize_or_zero()
}

/// The `i`-th of `n` points of the Hammersley low-discrepancy sequence (van der Corput
/// radical inverse in base 2 on the second axis). Deterministic — no RNG.
fn hammersley(i: u32, n: u32) -> Vec2 {
    Vec2::new(i as f32 / n as f32, radical_inverse_vdc(i))
}

/// Base-2 radical inverse: reverse the bits and scale to [0, 1). This is the van der
/// Corput sequence, the second Hammersley axis.
fn radical_inverse_vdc(bits: u32) -> f32 {
    bits.reverse_bits() as f32 * 2.328_306_4e-10
}
