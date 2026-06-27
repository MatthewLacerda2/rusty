//! src/render/reflection_prefilter.rs — GGX specular prefilter for reflection probes (#245).
//!
//! Turns a captured static cubemap (LDR RGBA8, mip0) into a prefiltered mip chain where
//! mip N stores the environment convolved for roughness `N / (mips-1)` — the standard
//! Karis split-sum specular pre-integration. At shading time the forward pass selects the
//! mip from a surface's roughness (`textureSampleLevel`), so a mirror reads the sharp base
//! level and a rough surface reads a progressively blurrier one.
//!
//! The convolution is GGX importance sampling: for each output texel we treat its world
//! direction as both the surface normal and the view (the split-sum assumption N = V = R),
//! draw a low-discrepancy set of half-vectors weighted toward the GGX lobe, reflect the
//! view across each to get a light direction, and average the source cubemap there
//! weighted by `N·L`. mip0 is left a faithful copy (roughness 0 = a mirror).
//!
//! Pure CPU math over the captured bytes — no GPU, no wall-clock / RNG (the Hammersley
//! sequence is deterministic), so it is clean under the determinism guard like the
//! capture and SH projection it stands beside.

use std::f32::consts::PI;

use glam::{Vec2, Vec3};

use super::cube_sample::{linear_to_srgb, sample_linear, FaceProjections};
use super::{CubemapCapture, CubemapFace};

/// Number of importance samples per output texel for the rough mips. A few hundred taps
/// gives a smooth lobe at the small cubemap sizes a probe uses; raising it trades bake
/// time for less sampling noise.
const SAMPLE_COUNT: u32 = 128;

/// A prefiltered cubemap: a chain of mips, each six LDR RGBA8 faces. `mips[0]` is the
/// sharp base (roughness 0); each later mip halves the edge length and raises roughness.
/// This is exactly the layout the KTX2 encoder writes (largest level first).
pub struct PrefilteredCubemap {
    /// `mips[level][face]` is `mip_size(level)² * 4` bytes, row-major top-to-bottom, in
    /// `CubemapFace::ALL` order.
    pub mips: Vec<[Vec<u8>; 6]>,
    /// Base (mip 0) edge length in pixels.
    pub base_size: u32,
}

impl PrefilteredCubemap {
    /// Number of mip levels.
    pub fn level_count(&self) -> usize {
        self.mips.len()
    }

    /// Edge length of mip `level` (each level halves, floored at 1).
    pub fn level_size(&self, level: usize) -> u32 {
        (self.base_size >> level).max(1)
    }
}

/// The fractional mip level a `roughness` in [0, 1] selects from a `level_count`-deep
/// chain: `roughness * (level_count - 1)`, so roughness 0 reads the sharp base level and
/// roughness 1 the roughest. This MIRRORS the forward shader's `textureSampleLevel`
/// argument (`shader.wgsl`, env-reflection block), so the headless mapping and the GPU
/// agree — the unit-testable form of the shader's mip selection (#245).
pub fn roughness_to_mip(roughness: f32, level_count: u32) -> f32 {
    roughness.clamp(0.0, 1.0) * (level_count.max(1) - 1) as f32
}

/// Validate a prefiltered cube's mip layout: every level's six faces must be
/// `level_size² * 4` bytes. The encoder relies on this; exposing it keeps the chain's
/// shape (and the `level_size`/`roughness_to_mip` accessors) a checkable invariant rather
/// than an implicit assumption.
pub fn validate_layout(cube: &PrefilteredCubemap) -> bool {
    (0..cube.level_count()).all(|level| {
        let size = cube.level_size(level) as usize;
        cube.mips[level].iter().all(|f| f.len() == size * size * 4)
    })
}

/// Mip count for a base edge length: a full chain down to 1×1 (`log2(size) + 1`), so the
/// roughest mip is a single texel — the fully-diffuse limit of the lobe.
pub fn mip_count(base_size: u32) -> u32 {
    32 - base_size.max(1).leading_zeros()
}

/// GGX-prefilter a captured cubemap into a roughness mip chain. mip0 copies the capture
/// (a mirror); each rougher mip is importance-sampled from mip0's faces.
pub fn prefilter(capture: &CubemapCapture) -> PrefilteredCubemap {
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
                let (u, v) = texel_ndc(px, py, size);
                let n = proj.direction(face, u, v);
                let color = convolve(proj, src, src_res, n, roughness);
                write_texel(&mut out, px, py, size, color);
            }
        }
        out
    })
}

/// Face-NDC in [-1, 1] for pixel `(px, py)` of a `size`-wide face (`u` right, `v` up;
/// image rows top-to-bottom). Matches `probe_bake::face_uv`.
fn texel_ndc(px: u32, py: u32, size: u32) -> (f32, f32) {
    let u = ((px as f32 + 0.5) / size as f32) * 2.0 - 1.0;
    let v = 1.0 - ((py as f32 + 0.5) / size as f32) * 2.0;
    (u, v)
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

#[cfg(test)]
#[path = "reflection_prefilter_tests.rs"]
mod reflection_prefilter_tests;
