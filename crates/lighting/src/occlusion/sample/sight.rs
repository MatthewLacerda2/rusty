use glam::Vec3;

use super::baking::{Baking, REACH_SLACK};
use super::eye::Eye;
use super::targets::Chunk;

impl Baking {
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
