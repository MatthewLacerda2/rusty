use glam::{Mat4, Vec3, Vec4, Vec4Swizzles};

use crate::ibl::cubemap_capture::face_view_projection;
use crate::CubemapFace;

/// Built from `face_view_projection`, the stored layout: a hand-rolled basis would desync.
pub struct FaceProjections {
    view_proj: [Mat4; 6],
    inv_view_proj: [Mat4; 6],
}

impl FaceProjections {
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

    /// Feeds `reflection_prefilter::gpu`'s uniform, whose shader maps directions as this does.
    pub(crate) fn matrices(&self) -> (&[Mat4; 6], &[Mat4; 6]) {
        (&self.view_proj, &self.inv_view_proj)
    }

    pub fn direction(&self, face: CubemapFace, u: f32, v: f32) -> Vec3 {
        let clip = Vec4::new(u, v, 1.0, 1.0);
        let world = self.inv_view_proj[face as usize] * clip;
        (world.xyz() / world.w).normalize_or_zero()
    }

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

/// Clip-space NDC in [-1, 1]: `u` right, `v` up, so the top image row maps to `v ≈ +1`.
pub fn face_uv(px: u32, py: u32, res: u32) -> (f32, f32) {
    let u = ((px as f32 + 0.5) / res as f32) * 2.0 - 1.0;
    let v = 1.0 - ((py as f32 + 0.5) / res as f32) * 2.0;
    (u, v)
}

#[cfg(test)]
pub fn sample(proj: &FaceProjections, faces: &[Vec<Vec3>; 6], res: u32, dir: Vec3) -> Vec3 {
    let Some((face, u, v)) = proj.locate(dir) else {
        return Vec3::ZERO;
    };
    let fx = ((u * 0.5 + 0.5) * res as f32).floor();
    let fy = ((1.0 - (v * 0.5 + 0.5)) * res as f32).floor();
    let px = (fx as i64).clamp(0, res as i64 - 1) as u32;
    let py = (fy as i64).clamp(0, res as i64 - 1) as u32;
    faces[face as usize][(py * res + px) as usize]
}

/// The CPU mirror of `capture` in `engine/shaders/ibl/probe_prefilter.wgsl`: change both together.
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
