//! What one sampling cube sees of one object (#1714): whether a ray from one of
//! its points reaches one of the object's targets with no other object's
//! occluder turned toward it in between.
//!
//! The rays are what cost, and on a level of rooms almost all of them stop at
//! a wall near the eye. So before any ray, polygons try to prove the answer
//! (`shaft`), cheapest first: one that hides the whole object from the whole
//! eye; then, per point, one that hides the object from that point, so points
//! hidden by different walls fuse; then the same per chunk of targets
//! (`targets::chunks`). Only then does a ray go out, through the remembered
//! polygons (`eye`) before the BVH. Every proof is exact, so the bake sees
//! what one ray per point and target would.

use glam::Vec3;

use super::baking::{Baking, REACH_SLACK};
use super::eye::Eye;
use super::targets::Chunk;

impl Baking {
    /// Whether `eye` sees `object` (its index and bounds), whose targets are
    /// `chunks`.
    pub(super) fn sees(
        &self,
        eye: &mut Eye,
        (object, bounds): (u32, (Vec3, Vec3)),
        chunks: &[Chunk],
    ) -> bool {
        if self.proved(eye, eye.bounds, bounds, object) {
            return false;
        }
        let polygons = &self.polygons;
        let open = eye.open(polygons, eye.all(), bounds, object);
        for chunk in chunks {
            let open = eye.open(polygons, open, chunk.bounds, object);
            let Some(from) = eye.box_of(open) else {
                continue;
            };
            for target in chunk.targets.iter().filter(|t| t.faces(from)) {
                for i in Eye::each(open) {
                    let origin = eye.points[i];
                    if target.drawn_toward(origin) && self.ray(eye, origin, target.at, object) {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Whether one polygon hides box `to` (on object `object`, `u32::MAX` for
    /// none) from box `from` of `eye`: a remembered one, else the one the BVH
    /// meets between their centres, which is then remembered.
    pub(super) fn proved(
        &self,
        eye: &mut Eye,
        from: (Vec3, Vec3),
        to: (Vec3, Vec3),
        object: u32,
    ) -> bool {
        let polygons = &self.polygons;
        if eye.cached(polygons, from, to, object) {
            return true;
        }
        let hider = polygons.hider(&self.rays, from, to, object);
        hider.inspect(|&p| eye.remember(p)).is_some()
    }

    /// Whether the ray from `origin` reaches `to`, a target on `object`: no
    /// remembered polygon, and no triangle of another object turned toward it,
    /// stands in between. The triangle that stops it is remembered.
    fn ray(&self, eye: &mut Eye, origin: Vec3, to: Vec3, object: u32) -> bool {
        let delta = to - origin;
        let reach = delta.length();
        if reach <= REACH_SLACK {
            return true;
        }
        if eye.cached(&self.polygons, (origin, origin), (to, to), object) {
            return false;
        }
        let dir = delta / reach;
        let blocks = |t: usize| {
            let tri = &self.rays.tris[t];
            tri.mesh != object && self.faces[t].stops(tri.normal(), dir)
        };
        match (self.rays).blocker(origin, dir, (0.0, reach - REACH_SLACK), blocks) {
            Some(tri) => {
                eye.remember(self.polygons.polygon_of(tri));
                false
            }
            None => true,
        }
    }
}
