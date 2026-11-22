use glam::Vec3;

use super::super::rng::Rng;
use super::{cosine_sample, Tracer, SURFACE_OFFSET, T_MIN};
use rusty_scene::lighting::settings::BakeSettings;

/// Fixed per texel whatever the sample count, so validity never flips with it.
pub(in crate::lightmap) const PROBES: u32 = 32;

impl Tracer<'_> {
    pub(in crate::lightmap) fn valid(
        &self,
        position: Vec3,
        normal: Vec3,
        tolerance: f32,
        rng: &mut Rng,
    ) -> bool {
        let tolerance = BakeSettings::backface_tolerance_of(tolerance);
        if tolerance <= 0.0 {
            return true;
        }
        let origin = position + normal * SURFACE_OFFSET;
        let back = (0..PROBES)
            .filter(|_| {
                let dir = cosine_sample(normal, rng);
                let hit = self.rays.closest(origin, dir, T_MIN, f32::MAX);
                hit.is_some_and(|h| self.rays.tris[h.tri].normal().dot(dir) >= 0.0)
            })
            .count() as f32;
        PROBES as f32 - back >= tolerance * PROBES as f32
    }
}
