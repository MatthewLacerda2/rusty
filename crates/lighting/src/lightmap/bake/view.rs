use glam::{Mat4, Vec3};

use super::super::progress::BakeProgress;
use super::super::raster::TexelPoint;
use super::super::rays::Rays;
use super::super::trace::T_MIN;
use super::pool;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BakeView {
    pub eye: Vec3,
    pub view_proj: Mat4,
}

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
        if self.view_proj.row(3) != glam::Vec4::W {
            let to = self.eye - p;
            let distance = to.length();
            return Some((to / distance.max(f32::MIN_POSITIVE), distance));
        }
        let ahead = (self.view_proj.inverse() * glam::Vec4::Z).truncate();
        let back = -ahead.try_normalize()?;
        let rate = (self.view_proj * (-back).extend(0.0)).z;
        (rate > 0.0).then(|| (back, depth / rate))
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
