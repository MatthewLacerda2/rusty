//! The nearest walkable point (#458): Unity's `NavMesh.SamplePosition`.

use glam::Vec3;

use super::super::{in_mask, NavigationGraph, ALL_AREAS};

impl NavigationGraph {
    /// The walkable point nearest `point` (straight-line 3D distance) within
    /// `max_distance`, or `None`. Each span covers its cell's square at its floor
    /// height, so the answer is `point` clamped into the nearest span's cell, on its
    /// floor. Ties keep the first span in storage order, so the answer is deterministic.
    pub fn sample_position(&self, point: Vec3, max_distance: f32) -> Option<Vec3> {
        self.sample_position_masked(point, max_distance, ALL_AREAS)
    }

    /// [`Self::sample_position`] over the spans whose area is in `mask` (#460).
    pub fn sample_position_masked(
        &self,
        point: Vec3,
        max_distance: f32,
        mask: u32,
    ) -> Option<Vec3> {
        if max_distance.is_nan() || max_distance < 0.0 || !point.is_finite() {
            return None;
        }
        let rings = (max_distance / self.grid_spacing).ceil();
        let rings = rings.min(self.width.max(self.height) as f32) as i32;
        let (cx, cz) = self.world_to_grid(point);
        let max_sq = max_distance * max_distance;
        let mut best: Option<(f32, Vec3)> = None;
        for gz in cz - rings..=cz + rings {
            for gx in cx - rings..=cx + rings {
                let flat = self.clamp_to_cell(point, gx, gz);
                for span in self
                    .spans_at(gx, gz)
                    .iter()
                    .filter(|s| in_mask(mask, s.area))
                {
                    let p = Vec3::new(flat.x, span.y, flat.z);
                    let d = p.distance_squared(point);
                    if d <= max_sq && best.is_none_or(|(b, _)| d < b) {
                        best = Some((d, p));
                    }
                }
            }
        }
        best.map(|(_, p)| p)
    }
}
