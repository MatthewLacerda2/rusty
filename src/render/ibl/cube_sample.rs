//! src/render/cube_sample.rs — CPU-side cubemap direction <-> texel mapping (#245).
//!
//! The GGX prefilter (`reflection_prefilter.rs`) needs two pure operations over a
//! captured cubemap: the world DIRECTION each output texel points down, and the COLOUR
//! the captured environment shows along an arbitrary direction. Both must agree with the
//! exact per-face orientation `capture_static_cubemap` rendered with (#243) — the faces
//! carry the empirically-tuned flipped cameras, so a hand-rolled axis basis would desync.
//!
//! So this module derives everything from the same `face_camera` view-projection the
//! capture used: [`FaceProjections`] inverts it for texel->direction (mirroring
//! `probe_bake::FaceBasis`) and uses the forward matrix for direction->texel, picking the
//! face whose frustum a direction falls inside. Pure math — no GPU, no clock/RNG.

use glam::{Mat4, Vec3, Vec4, Vec4Swizzles};

use crate::render::ibl::cubemap_capture::face_camera;
use crate::render::CubemapFace;

/// The six faces' forward + inverse view-projection matrices, cached once so the
/// prefilter can map directions and texels without rebuilding cameras per sample.
pub struct FaceProjections {
    view_proj: [Mat4; 6],
    inv_view_proj: [Mat4; 6],
}

impl FaceProjections {
    /// Build the per-face matrices for a cube captured at the origin (directions are
    /// position-independent, so the origin is the right pivot — exactly what the SH
    /// projection uses in `probe_bake::FaceBasis`).
    pub fn new() -> Self {
        let mut view_proj = [Mat4::IDENTITY; 6];
        let mut inv_view_proj = [Mat4::IDENTITY; 6];
        for face in CubemapFace::ALL {
            let camera = face_camera(Vec3::ZERO, face);
            let vp = camera.build_view_projection(1.0);
            view_proj[face as usize] = vp;
            inv_view_proj[face as usize] = vp.inverse();
        }
        Self {
            view_proj,
            inv_view_proj,
        }
    }

    /// World direction for the texel at face coords `(u, v)` in [-1, 1] (clip-space NDC,
    /// x right, y up). Mirrors `probe_bake::FaceBasis::direction` (origin eye).
    pub fn direction(&self, face: CubemapFace, u: f32, v: f32) -> Vec3 {
        let clip = Vec4::new(u, v, 1.0, 1.0);
        let world = self.inv_view_proj[face as usize] * clip;
        (world.xyz() / world.w).normalize_or_zero()
    }

    /// The (face, u, v) a world `dir` projects to: the face whose frustum it falls inside
    /// (positive clip w, NDC within [-1, 1]). Returns `None` only for the zero vector.
    fn locate(&self, dir: Vec3) -> Option<(CubemapFace, f32, f32)> {
        let dir = dir.normalize_or_zero();
        if dir == Vec3::ZERO {
            return None;
        }
        for face in CubemapFace::ALL {
            let clip = self.view_proj[face as usize] * dir.extend(0.0);
            if clip.w <= 0.0 {
                continue;
            }
            let ndc = clip.xy() / clip.w;
            if ndc.x.abs() <= 1.0 && ndc.y.abs() <= 1.0 {
                return Some((face, ndc.x, ndc.y));
            }
        }
        None
    }
}

impl Default for FaceProjections {
    fn default() -> Self {
        Self::new()
    }
}

/// Sample the LDR RGBA8 faces of a captured cubemap along `dir`, returning linear RGB
/// (sRGB-decoded). Nearest-texel lookup is plenty for the importance-sampled prefilter,
/// which averages many taps. Black when the direction can't be located (only the zero
/// vector). `faces[f]` is `res * res * 4` bytes, row-major top-to-bottom.
pub fn sample_linear(proj: &FaceProjections, faces: &[Vec<u8>; 6], res: u32, dir: Vec3) -> Vec3 {
    let Some((face, u, v)) = proj.locate(dir) else {
        return Vec3::ZERO;
    };
    // NDC (u right, v up) -> pixel (px right, py down); inverse of `probe_bake::face_uv`.
    let fx = ((u * 0.5 + 0.5) * res as f32).floor();
    let fy = ((1.0 - (v * 0.5 + 0.5)) * res as f32).floor();
    let px = (fx as i64).clamp(0, res as i64 - 1) as u32;
    let py = (fy as i64).clamp(0, res as i64 - 1) as u32;
    let idx = ((py * res + px) * 4) as usize;
    let pixels = &faces[face as usize];
    Vec3::new(
        srgb_to_linear(pixels[idx]),
        srgb_to_linear(pixels[idx + 1]),
        srgb_to_linear(pixels[idx + 2]),
    )
}

/// Inverse-sRGB transfer for one 8-bit channel (matches `probe_bake::srgb_to_linear`).
pub fn srgb_to_linear(b: u8) -> f32 {
    let c = b as f32 / 255.0;
    if c <= 0.040_45 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// Forward-sRGB transfer: linear channel -> 8-bit byte, the encode side of the prefilter
/// (the output KTX2 stores sRGB-encoded LDR, the same space the capture produced).
pub fn linear_to_srgb(c: f32) -> u8 {
    let c = c.clamp(0.0, 1.0);
    let s = if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    };
    (s * 255.0 + 0.5).clamp(0.0, 255.0) as u8
}

#[cfg(test)]
#[path = "cube_sample_tests.rs"]
mod cube_sample_tests;
