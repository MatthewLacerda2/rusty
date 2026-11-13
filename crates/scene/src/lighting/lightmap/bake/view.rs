//! Unity's *Prioritize View* for a progressive bake (#1970): which batches of
//! texels the Scene view's camera sees, so they converge first.
//!
//! A texel is seen when it lies in the camera's frustum, faces the camera (its
//! eye, or for an orthographic view its view direction) and nothing static the
//! camera draws stands between them (one ray per texel, cast once before the
//! first pass). A batch is seen when any of its texels is. Only when the
//! batches are traced moves (`progressive::schedule`), never what a texel draws,
//! so the bytes are the same with or without a view.

use glam::{Mat4, Vec3};

use super::super::progress::BakeProgress;
use super::super::raster::TexelPoint;
use super::super::rays::Rays;
use super::super::trace::T_MIN;
use super::pool;

/// The camera a progressive bake prioritizes: where it is and what it sees.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BakeView {
    /// The camera's position in the world.
    pub eye: Vec3,
    /// Its projection times its view: world to clip space, depth 0 to 1.
    pub view_proj: Mat4,
}

/// Ray offset off the texel, against hitting its own surface.
const LIFT: f32 = 1e-3;

impl BakeView {
    /// Whether the camera sees the texel at `p` facing `normal` past `rays`. Only
    /// what the camera draws hides it: a `ShadowsOnly` mesh does not (#1571).
    pub(in crate::lighting::lightmap) fn sees(&self, rays: &Rays, p: Vec3, normal: Vec3) -> bool {
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

    /// The way from `p` (at clip depth `depth`) back to the camera, and how far:
    /// to the eye for a perspective view, along the view direction to the near
    /// plane for an orthographic one (its projection leaves `w` at 1).
    fn toward(&self, p: Vec3, depth: f32) -> Option<(Vec3, f32)> {
        if self.view_proj.row(3) != glam::Vec4::W {
            let to = self.eye - p;
            let distance = to.length();
            return Some((to / distance.max(f32::MIN_POSITIVE), distance));
        }
        // Clip depth grows away from the camera by `rate` per unit along `back`'s
        // opposite: walk back until it is 0.
        let ahead = (self.view_proj.inverse() * glam::Vec4::Z).truncate();
        let back = -ahead.try_normalize()?;
        let rate = (self.view_proj * (-back).extend(0.0)).z;
        (rate > 0.0).then(|| (back, depth / rate))
    }

    /// Per chart, per batch of `pool::BATCH` texels, whether the camera sees any
    /// of them (none for a chart with no texel): the charts' batches as the
    /// bake's pool cuts them, checked on it on `threads` workers. `None` when `progress` is cancelled first.
    pub(super) fn seen(
        &self,
        rays: &Rays,
        charts: &[Vec<TexelPoint>],
        threads: usize,
        progress: &BakeProgress,
    ) -> Option<Vec<Vec<bool>>> {
        let texels: usize = charts.iter().map(Vec::len).sum();
        let charts = charts.iter().map(|c| c.iter().collect()).collect();
        // A chart with no texel gets the pool's one empty batch: it says nothing,
        // so a view that sees every real texel is no partial view (#1970's review).
        let batch = |_: usize, _: usize, points: &[&TexelPoint]| match points.is_empty() {
            true => Vec::new(),
            false => vec![points.iter().any(|p| self.sees(rays, p.position, p.normal))],
        };
        let seen = pool::run(charts, threads, progress, batch, |_, _, seen| seen);
        // The pool counted the texels it looked at; none was traced yet.
        progress.take_back(texels as u64);
        seen
    }
}
