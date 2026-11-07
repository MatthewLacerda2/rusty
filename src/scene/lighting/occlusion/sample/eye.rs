//! One sampling cube of a cell, as the occlusion bake traces from it (#1714,
//! part 3): its kept points, the box round them, and the occluder polygons that
//! last hid something from it.
//!
//! Most of a Dust2-sized bake's rays are stopped by the few walls round the eye,
//! so each cube keeps the polygons that last stopped one ([`CACHED`] of them, most
//! recent first) and tries them before the BVH. A polygon only ever proves a
//! segment or a box hidden, exactly as the BVH would (`shaft`), so the cache
//! changes how fast a cell bakes, never what it sees. Each cell is baked in one
//! fixed order on one thread, so the cache is deterministic too.

use glam::Vec3;

use super::shaft::Polygons;

/// How many polygons a cube remembers.
const CACHED: usize = 8;

/// A sampling cube: its points and the box round them, which every shaft test
/// uses (rays only ever leave from the points, so a tighter box proves more).
pub(super) struct Eye {
    pub points: Vec<Vec3>,
    pub bounds: (Vec3, Vec3),
    /// Polygon ids, most recent first.
    cache: Vec<u32>,
}

impl Eye {
    /// The eye of `points`, which are never empty (`baking` keeps a cube only
    /// with a point left).
    pub fn new(points: Vec<Vec3>) -> Self {
        Self {
            bounds: Self::box_of(&points).unwrap_or_default(),
            points,
            cache: Vec::with_capacity(CACHED),
        }
    }

    /// The box round `points`, `None` when there are none.
    pub fn box_of(points: &[Vec3]) -> Option<(Vec3, Vec3)> {
        let lo = points.iter().copied().reduce(Vec3::min)?;
        let hi = points.iter().copied().reduce(Vec3::max)?;
        Some((lo, hi))
    }

    /// A remembered polygon that hides box `to` (on object `object`) from the
    /// box `from`, moved to the front.
    pub fn cached(
        &mut self,
        polygons: &Polygons,
        from: (Vec3, Vec3),
        to: (Vec3, Vec3),
        object: u32,
    ) -> bool {
        let found = self
            .cache
            .iter()
            .position(|&p| polygons.hides(p, from, to, object));
        found
            .inspect(|&at| self.cache[..=at].rotate_right(1))
            .is_some()
    }

    /// Those of `points` from which no remembered polygon hides box `to` (on
    /// object `object`).
    pub fn open(
        &mut self,
        polygons: &Polygons,
        points: &[Vec3],
        to: (Vec3, Vec3),
        object: u32,
    ) -> Vec<Vec3> {
        let hidden = |eye: &mut Self, p: Vec3| eye.cached(polygons, (p, p), to, object);
        points
            .iter()
            .copied()
            .filter(|&p| !hidden(self, p))
            .collect()
    }

    /// Remember `polygon` as the latest that hid something.
    pub fn remember(&mut self, polygon: u32) {
        match self.cache.iter().position(|&p| p == polygon) {
            Some(at) => self.cache[..=at].rotate_right(1),
            None => {
                self.cache.truncate(CACHED - 1);
                self.cache.insert(0, polygon);
            }
        }
    }
}
