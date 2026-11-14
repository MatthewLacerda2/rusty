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

/// Some of an eye's points, bit `i` for point `i`: an eye has at most eight.
pub(super) type Points = u8;

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
    /// with a point left) and at most eight.
    pub fn new(points: Vec<Vec3>) -> Self {
        assert!(
            points.len() <= Points::BITS as usize,
            "an eye has at most 8 points"
        );
        let mut eye = Self {
            bounds: (Vec3::ZERO, Vec3::ZERO),
            points,
            cache: Vec::with_capacity(CACHED),
        };
        eye.bounds = eye.box_of(eye.all()).unwrap_or_default();
        eye
    }

    /// All its points.
    pub fn all(&self) -> Points {
        ((1u16 << self.points.len()) - 1) as Points
    }

    /// The indices of `points`, ascending.
    pub fn each(points: Points) -> impl Iterator<Item = usize> {
        (0..Points::BITS as usize).filter(move |&i| points & (1 << i) != 0)
    }

    /// The box round `points`, `None` when there are none.
    pub fn box_of(&self, points: Points) -> Option<(Vec3, Vec3)> {
        let mut at = Self::each(points).map(|i| self.points[i]);
        let first = at.next()?;
        Some(at.fold((first, first), |(lo, hi), p| (lo.min(p), hi.max(p))))
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
        points: Points,
        to: (Vec3, Vec3),
        object: u32,
    ) -> Points {
        let mut open = points;
        for i in Self::each(points) {
            let p = self.points[i];
            if self.cached(polygons, (p, p), to, object) {
                open &= !(1 << i);
            }
        }
        open
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
