//! Whether a texel sits inside geometry (#1949), Unity's *Backface Tolerance*.
//!
//! A texel under a crate or inside a wall sees mostly back faces: its light is
//! meaningless, so the bake drops it and fills it from its own chart
//! (`raster::charts`). The test is its own fixed budget of [`PROBES`] rays per
//! texel from its own seeded stream ([`Rng::probes`]), never the paths that gather
//! light: so a texel near the threshold is valid or not the same way at 16 paths
//! and at 4096, and does not flip as the sample count changes.

use glam::Vec3;

use super::super::rng::Rng;
use super::{cosine_sample, Tracer, SURFACE_OFFSET, T_MIN};

/// Probe rays per texel that decide its validity, whatever the bake's samples.
pub(in crate::scene::lighting::lightmap) const PROBES: u32 = 64;

impl Tracer<'_> {
    /// Whether the texel at `position` facing `normal` is valid: at least
    /// `tolerance` of its [`PROBES`] cosine-weighted rays reach a front face or
    /// escape (Unity counts a texel valid by the share of its rays that see front
    /// faces; a ray that meets nothing sees no back face either). `tolerance` is
    /// clamped to `[0, 1]`: 0 keeps every texel, 1 drops one that sees any back face.
    pub(in crate::scene::lighting::lightmap) fn valid(
        &self,
        position: Vec3,
        normal: Vec3,
        tolerance: f32,
        rng: &mut Rng,
    ) -> bool {
        let origin = position + normal * SURFACE_OFFSET;
        let back = (0..PROBES)
            .filter(|_| {
                let dir = cosine_sample(normal, rng);
                let hit = self.rays.closest(origin, dir, T_MIN, f32::MAX);
                hit.is_some_and(|h| self.rays.tris[h.tri].normal().dot(dir) >= 0.0)
            })
            .count() as f32;
        let tolerance = if tolerance.is_nan() {
            0.0
        } else {
            tolerance.clamp(0.0, 1.0)
        };
        PROBES as f32 - back >= tolerance * PROBES as f32
    }
}
