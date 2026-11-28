use glam::{Mat4, Vec3};
use rusty_scene::camera::depth::{FAR, NEAR};

use super::super::progress::BakeProgress;
use super::super::raster::TexelPoint;
use super::super::rays::Rays;
use super::super::trace::T_MIN;
use super::pool;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BakeView {
    pub eye: Vec3,
    /// World to clip, with a camera's reversed depth (`rusty_scene::camera::depth`).
    pub view_proj: Mat4,
}

/// World units off the texel, so its ray misses its own surface.
const LIFT: f32 = 1e-3;

impl BakeView {
    pub(in crate::lightmap) fn sees(&self, rays: &Rays, p: Vec3, normal: Vec3) -> bool {
        let clip = self.view_proj * p.extend(1.0);
        let w = clip.w;
        let inside =
            w > 0.0 && clip.x.abs() <= w && clip.y.abs() <= w && (0.0..=w).contains(&clip.z);
        let Some((to, distance)) = inside.then(|| self.toward(p, clip.z)).flatten() else {
            return false;
        };
        if normal.dot(to) <= 0.0 {
            return false;
        }
        let from = p + normal * LIFT;
        let drawn = |i: usize| rays.tris[i].seen;
        distance <= T_MIN || !rays.occluded_by(from, to, (T_MIN, distance), drawn)
    }

    fn toward(&self, p: Vec3, depth: f32) -> Option<(Vec3, f32)> {
        // An orthographic projection leaves `w` at 1: no eye, so back along the view.
        if self.view_proj.row(3) != glam::Vec4::W {
            let to = self.eye - p;
            let distance = to.length();
            return Some((to / distance.max(f32::MIN_POSITIVE), distance));
        }
        // Depth's own axis, run from the near plane toward the far one: the view direction.
        let axis = glam::Vec4::new(0.0, 0.0, FAR - NEAR, 0.0);
        let back = -(self.view_proj.inverse() * axis)
            .truncate()
            .try_normalize()?;
        // Depth gained per unit walked back toward the near plane, and the walk to it.
        let rate = (self.view_proj * back.extend(0.0)).z;
        let distance = (NEAR - depth) / rate;
        (rate != 0.0 && distance >= 0.0).then_some((back, distance))
    }

    pub(super) fn seen(
        &self,
        rays: &Rays,
        charts: &[Vec<TexelPoint>],
        threads: usize,
        progress: &BakeProgress,
    ) -> Option<Vec<Vec<bool>>> {
        let texels: usize = charts.iter().map(Vec::len).sum();
        let charts = charts.iter().map(|c| c.iter().collect()).collect();
        let batch = |_: usize, _: usize, points: &[&TexelPoint]| match points.is_empty() {
            true => Vec::new(),
            false => vec![points.iter().any(|p| self.sees(rays, p.position, p.normal))],
        };
        let seen = pool::run(charts, threads, progress, batch, |_, _, seen| seen);
        progress.take_back(texels as u64);
        seen
    }
}
