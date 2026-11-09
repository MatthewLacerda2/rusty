//! Unity's *Prioritize View* for a progressive bake (#1970): which batches of
//! texels the Scene view's camera sees, so each pass traces them first.
//!
//! A texel is seen when it lies in the camera's frustum, faces the camera and
//! nothing static stands between them (one ray per texel, cast once before the
//! first pass). A batch is seen when any of its texels is. Only the order of the
//! batches moves, never what a texel draws, so the bytes are the same with or
//! without a view.

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
    /// Whether the camera sees the texel at `p` facing `normal` past `rays`.
    pub(in crate::scene::lighting::lightmap) fn sees(
        &self,
        rays: &Rays,
        p: Vec3,
        normal: Vec3,
    ) -> bool {
        let clip = self.view_proj * p.extend(1.0);
        let w = clip.w;
        let inside =
            w > 0.0 && clip.x.abs() <= w && clip.y.abs() <= w && (0.0..=w).contains(&clip.z);
        if !inside || normal.dot(self.eye - p) <= 0.0 {
            return false;
        }
        let from = p + normal * LIFT;
        let to = self.eye - from;
        let distance = to.length();
        distance <= T_MIN || !rays.occluded(from, to / distance, T_MIN, distance)
    }

    /// Per chart, per batch of `pool::BATCH` texels, whether the camera sees any
    /// of them: the charts' batches as the bake's pool cuts them, checked on it on
    /// `threads` workers. `None` when `progress` is cancelled first.
    pub(super) fn seen(
        &self,
        rays: &Rays,
        charts: &[Vec<TexelPoint>],
        threads: usize,
        progress: &BakeProgress,
    ) -> Option<Vec<Vec<bool>>> {
        let texels: usize = charts.iter().map(Vec::len).sum();
        let charts = charts.iter().map(|c| c.iter().collect()).collect();
        let batch = |_: usize, _: usize, points: &[&TexelPoint]| {
            vec![points.iter().any(|p| self.sees(rays, p.position, p.normal))]
        };
        let seen = pool::run(charts, threads, progress, batch, |_, _, seen| seen);
        // The pool counted the texels it looked at; none was traced yet.
        progress.take_back(texels as u64);
        seen
    }
}
