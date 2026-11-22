use glam::Vec3;

use super::shaft::Polygons;

const CACHED: usize = 8;

pub(super) type Points = u8;

pub(super) struct Eye {
    pub points: Vec<Vec3>,
    pub bounds: (Vec3, Vec3),
    cache: Vec<u32>,
}

impl Eye {
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

    pub fn all(&self) -> Points {
        ((1u16 << self.points.len()) - 1) as Points
    }

    pub fn each(points: Points) -> impl Iterator<Item = usize> {
        (0..Points::BITS as usize).filter(move |&i| points & (1 << i) != 0)
    }

    pub fn box_of(&self, points: Points) -> Option<(Vec3, Vec3)> {
        let mut at = Self::each(points).map(|i| self.points[i]);
        let first = at.next()?;
        Some(at.fold((first, first), |(lo, hi), p| (lo.min(p), hi.max(p))))
    }

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
