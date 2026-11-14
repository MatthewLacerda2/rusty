//! crates/render/src/ibl/cube_sample.rs — CPU-side cubemap direction <-> texel mapping (#245).
//!
//! The CPU GGX prefilter (`reflection_prefilter::cpu`, the reference the bake's GPU
//! one is held to, #1602) needs two pure operations over a
//! captured cubemap: the world DIRECTION each output texel points down, and the RADIANCE
//! the captured environment shows along an arbitrary direction. Both must agree with the
//! exact per-face orientation `capture_static_cubemap` stored (#243) — the faces
//! carry `face_camera`'s exact lenses (its up vectors and the readback flip included,
//! #1615), so a hand-rolled axis basis would desync.
//!
//! So this module derives everything from the same `face_view_projection` the
//! capture stored: [`FaceProjections`] inverts it for texel->direction (which
//! `probe_bake` also uses for its SH projection) and uses the forward matrix for direction->texel, picking the
//! face whose frustum a direction falls inside. Pure math — no GPU, no clock/RNG.

use glam::{Mat4, Vec3, Vec4, Vec4Swizzles};

use crate::ibl::cubemap_capture::face_view_projection;
use crate::CubemapFace;

/// The six faces' forward + inverse view-projection matrices, cached once so the
/// prefilter can map directions and texels without rebuilding cameras per sample.
pub struct FaceProjections {
    view_proj: [Mat4; 6],
    inv_view_proj: [Mat4; 6],
}

impl FaceProjections {
    /// Build the per-face matrices for a cube captured at the origin (directions are
    /// position-independent, so the origin is the right pivot — exactly what the SH
    /// projection in `probe_bake` uses).
    pub fn new() -> Self {
        let mut view_proj = [Mat4::IDENTITY; 6];
        let mut inv_view_proj = [Mat4::IDENTITY; 6];
        for face in CubemapFace::ALL {
            let vp = face_view_projection(Vec3::ZERO, face);
            view_proj[face as usize] = vp;
            inv_view_proj[face as usize] = vp.inverse();
        }
        Self {
            view_proj,
            inv_view_proj,
        }
    }

    /// Each face's view-projection and its inverse, for the GPU prefilter's uniform
    /// (`reflection_prefilter::gpu`, #1602), which maps directions as this does.
    pub(crate) fn matrices(&self) -> (&[Mat4; 6], &[Mat4; 6]) {
        (&self.view_proj, &self.inv_view_proj)
    }

    /// World direction for the texel at face coords `(u, v)` in [-1, 1] (clip-space NDC,
    /// x right, y up).
    pub fn direction(&self, face: CubemapFace, u: f32, v: f32) -> Vec3 {
        let clip = Vec4::new(u, v, 1.0, 1.0);
        let world = self.inv_view_proj[face as usize] * clip;
        (world.xyz() / world.w).normalize_or_zero()
    }

    /// The (face, u, v) a world `dir` projects to: the face whose frustum it falls inside
    /// (positive clip w, NDC within [-1, 1]). Returns `None` only for the zero vector.
    /// The CPU reference prefilter's; the GPU one locates in its shader (#1602).
    #[cfg(test)]
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

/// Face coords in [-1, 1] for pixel `(px, py)` of a `res`-wide face: clip-space NDC,
/// `u` right, `v` up (image rows are top-to-bottom, so the top row maps to `v ≈ +1`).
pub fn face_uv(px: u32, py: u32, res: u32) -> (f32, f32) {
    let u = ((px as f32 + 0.5) / res as f32) * 2.0 - 1.0;
    let v = 1.0 - ((py as f32 + 0.5) / res as f32) * 2.0;
    (u, v)
}

/// The captured radiance along `dir`: the nearest texel of `faces` (each `res²`
/// linear texels, row-major top-to-bottom). Nearest-texel lookup is plenty for the
/// importance-sampled prefilter, which averages many taps, each from the source mip
/// its solid angle picks. Black when the direction can't be located (only the zero
/// vector).
#[cfg(test)]
pub fn sample(proj: &FaceProjections, faces: &[Vec<Vec3>; 6], res: u32, dir: Vec3) -> Vec3 {
    let Some((face, u, v)) = proj.locate(dir) else {
        return Vec3::ZERO;
    };
    // NDC (u right, v up) -> pixel (px right, py down); inverse of [`face_uv`].
    let fx = ((u * 0.5 + 0.5) * res as f32).floor();
    let fy = ((1.0 - (v * 0.5 + 0.5)) * res as f32).floor();
    let px = (fx as i64).clamp(0, res as i64 - 1) as u32;
    let py = (fy as i64).clamp(0, res as i64 - 1) as u32;
    faces[face as usize][(py * res + px) as usize]
}

/// [`sample`] of a source mip chain (`chain[level]`, the base `base` texels wide) at
/// fractional level `lod`: the nearest texel of the two levels around it, blended.
/// `probe_prefilter.wgsl`'s `capture` is this.
#[cfg(test)]
pub fn sample_lod(
    proj: &FaceProjections,
    chain: &[[Vec<Vec3>; 6]],
    base: u32,
    dir: Vec3,
    lod: f32,
) -> Vec3 {
    let last = chain.len() - 1;
    let lo = (lod.floor() as usize).min(last);
    let hi = (lo + 1).min(last);
    let at = |level: usize| sample(proj, &chain[level], (base >> level).max(1), dir);
    at(lo).lerp(at(hi), lod - lo as f32)
}

#[cfg(test)]
#[path = "cube_sample_tests.rs"]
mod cube_sample_tests;
