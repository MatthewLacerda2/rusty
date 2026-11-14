//! The CPU GGX prefilter (#245): the reference the GPU one (`super::gpu`, #1602) is
//! held to, texel for texel. Only tests run it; the bake prefilters on the GPU.

use std::f32::consts::PI;

use glam::{Vec2, Vec3};

use super::source::{encode_face, sample_mip, source_chain};
use super::{mip_count, PrefilteredCubemap, SAMPLE_COUNT};
use crate::ibl::cube_sample::{face_uv, sample_lod, FaceProjections};
use crate::{CubemapCapture, CubemapFace};

/// GGX-prefilter a captured cubemap into a roughness mip chain. mip0 copies the capture
/// (a mirror); each rougher mip is importance-sampled from the capture's source mip
/// chain (`source::source_chain`), every sample at the level its solid angle picks.
pub(crate) fn prefilter(capture: &CubemapCapture) -> PrefilteredCubemap {
    let base = capture.resolution.max(1);
    let levels = mip_count(base);
    let proj = FaceProjections::new();
    let chain = source_chain(capture);
    let mut mips = Vec::with_capacity(levels as usize);

    // mip0: a faithful copy of the capture (roughness 0 = a perfect mirror).
    mips.push(chain[0].each_ref().map(|face| encode_face(face)));

    for level in 1..levels {
        let size = (base >> level).max(1);
        let roughness = level as f32 / (levels - 1) as f32;
        mips.push(filter_mip(&proj, &chain, base, size, roughness));
    }

    PrefilteredCubemap {
        mips,
        base_size: base,
    }
}

/// Build one rough mip: every texel of every face GGX-convolved from the source chain.
fn filter_mip(
    proj: &FaceProjections,
    chain: &[[Vec<Vec3>; 6]],
    base: u32,
    size: u32,
    roughness: f32,
) -> [Vec<u8>; 6] {
    CubemapFace::ALL.map(|face| {
        let texels: Vec<Vec3> = (0..size * size)
            .map(|i| {
                let (u, v) = face_uv(i % size, i / size, size);
                let n = proj.direction(face, u, v);
                convolve(proj, chain, base, n, (roughness, Taps::BAKE))
            })
            .collect();
        encode_face(&texels)
    })
}

/// How a convolution samples: how many taps, and whether each reads the source mip
/// its solid angle picks or the base level alone.
#[derive(Clone, Copy)]
pub(crate) struct Taps {
    pub count: u32,
    pub filtered: bool,
}

impl Taps {
    /// The bake's: [`SAMPLE_COUNT`] filtered taps.
    pub(crate) const BAKE: Taps = Taps {
        count: SAMPLE_COUNT,
        filtered: true,
    };
}

/// GGX split-sum convolution at normal `n` for `roughness`: importance-sample half-vectors
/// around `n`, reflect the view (= n) across each, and average the source radiance at the
/// sample's own mip, weighted by `N·L`. Returns linear RGB. Tests convolve with other
/// [`Taps`] for a ground truth to hold the bake's against.
pub(crate) fn convolve(
    proj: &FaceProjections,
    chain: &[[Vec<Vec3>; 6]],
    base: u32,
    n: Vec3,
    (roughness, taps): (f32, Taps),
) -> Vec3 {
    let (tangent, bitangent) = basis(n);
    let a = roughness * roughness;
    let mut color = Vec3::ZERO;
    let mut weight = 0.0;
    for i in 0..taps.count {
        let xi = hammersley(i, taps.count);
        let h = importance_sample_ggx(xi, roughness, n, tangent, bitangent);
        let l = (2.0 * n.dot(h) * h - n).normalize_or_zero();
        let ndotl = n.dot(l).max(0.0);
        if ndotl > 0.0 {
            let filtered = sample_mip(n.dot(h), a * a, taps.count, base);
            let lod = if taps.filtered { filtered } else { 0.0 };
            color += sample_lod(proj, chain, base, l, lod) * ndotl;
            weight += ndotl;
        }
    }
    if weight > 0.0 {
        color / weight
    } else {
        sample_lod(proj, chain, base, n, 0.0)
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
