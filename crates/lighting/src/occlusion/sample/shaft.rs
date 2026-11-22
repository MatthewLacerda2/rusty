use glam::Vec3;

use super::sheets::{sheets, Member, Sheet};
use super::{corners, opposite, CLEAR};
use crate::lightmap::{Rays, Tri};
use crate::occlusion::gather::Face;

pub(super) struct Polygons {
    of: Vec<u32>,
    polygons: Vec<Polygon>,
    sheets: Vec<Sheet>,
    sheet_of: Vec<u32>,
}

struct Polygon {
    corners: Vec<Vec3>,
    normal: Vec3,
    offset: f32,
    mesh: u32,
    face: Face,
}

impl Polygons {
    pub fn of(tris: &[Tri], corners: &[[Vec3; 3]], faces: &[Face]) -> Self {
        let (mut of, mut polygons) = (Vec::with_capacity(tris.len()), Vec::<Polygon>::new());
        for (i, (t, &corners)) in tris.iter().zip(corners).enumerate() {
            let joins = i > 0 && tris[i - 1].mesh == t.mesh && faces[i - 1] == faces[i];
            let last = polygons.last_mut().filter(|_| joins);
            if !last.is_some_and(|p| p.merge(corners)) {
                polygons.push(Polygon::new(corners, t.mesh, faces[i]));
            }
            of.push(polygons.len() as u32 - 1);
        }
        let members: Vec<Member> = (polygons.iter())
            .map(|p| Member {
                corners: &p.corners,
                normal: p.normal,
                mesh: p.mesh,
                face: p.face,
            })
            .collect();
        let (sheets, sheet_of) = sheets(&members);
        Self {
            of,
            polygons,
            sheets,
            sheet_of,
        }
    }

    pub fn hider(
        &self,
        rays: &Rays,
        from: (Vec3, Vec3),
        to: (Vec3, Vec3),
        object: u32,
    ) -> Option<u32> {
        let (c, t) = ((from.0 + from.1) * 0.5, (to.0 + to.1) * 0.5);
        let delta = t - c;
        let reach = delta.length();
        if reach.is_nan() || reach <= CLEAR {
            return None;
        }
        let hit = rays.closest(c, delta / reach, 0.0, reach)?;
        let polygon = self.of[hit.tri];
        self.hides(polygon, from, to, object).then_some(polygon)
    }

    pub fn hides(&self, polygon: u32, from: (Vec3, Vec3), to: (Vec3, Vec3), object: u32) -> bool {
        let sheet = self.sheets.get(self.sheet_of[polygon as usize] as usize);
        if sheet.is_some_and(|s| !s.holds(object) && s.hides(from, to)) {
            return true;
        }
        let p = &self.polygons[polygon as usize];
        let dir = (to.0 + to.1) * 0.5 - (from.0 + from.1) * 0.5;
        p.mesh != object && p.face.stops(p.normal, dir) && p.separates(from, to)
    }

    #[cfg(feature = "dev")]
    pub fn sheet_bytes(&self) -> (usize, usize) {
        (
            self.sheets.len(),
            self.sheets.iter().map(Sheet::bytes).sum(),
        )
    }

    pub fn polygon_of(&self, tri: usize) -> u32 {
        self.of[tri]
    }
}

impl Polygon {
    fn new(corners: [Vec3; 3], mesh: u32, face: Face) -> Self {
        let normal = (corners[1] - corners[0])
            .cross(corners[2] - corners[0])
            .normalize_or_zero();
        Self {
            corners: corners.to_vec(),
            normal,
            offset: normal.dot(corners[0]),
            mesh,
            face,
        }
    }

    fn merge(&mut self, t: [Vec3; 3]) -> bool {
        let n = (t[1] - t[0]).cross(t[2] - t[0]).normalize_or_zero();
        let flat = t
            .iter()
            .all(|&p| (self.normal.dot(p) - self.offset).abs() < CLEAR);
        if self.corners.len() != 3 || self.normal.dot(n) < 0.9999 || !flat {
            return false;
        }
        let same = |a: Vec3, b: Vec3| a.distance_squared(b) < 1e-10;
        for i in 0..3 {
            let (p, q) = (self.corners[i], self.corners[(i + 1) % 3]);
            let Some(k) = (0..3).find(|&k| same(t[k], q) && same(t[(k + 1) % 3], p)) else {
                continue;
            };
            let mut quad = self.corners.clone();
            quad.insert(i + 1, t[(k + 2) % 3]);
            let convex = (0..4).all(|j| {
                let (a, b, c) = (quad[j], quad[(j + 1) % 4], quad[(j + 2) % 4]);
                (b - a).cross(c - b).dot(self.normal) > 0.0
            });
            if convex {
                self.corners = quad;
            }
            return convex;
        }
        false
    }

    fn separates(&self, from: (Vec3, Vec3), to: (Vec3, Vec3)) -> bool {
        if self.normal == Vec3::ZERO {
            return false;
        }
        let side = |p: Vec3| self.normal.dot(p) - self.offset;
        let clear = opposite(self.normal, (self.offset, self.offset), from, to);
        let ((near, n), (far, f)) = (corners(from), corners(to));
        let (near, far) = (&near[..n], &far[..f]);
        clear
            && near.iter().all(|&a| {
                far.iter().all(|&b| {
                    let (da, db) = (side(a), side(b));
                    self.holds(a + (b - a) * (da / (da - db)))
                })
            })
    }

    fn holds(&self, x: Vec3) -> bool {
        let k = self.corners.len();
        (0..k).all(|i| {
            let (a, b) = (self.corners[i], self.corners[(i + 1) % k]);
            let edge = b - a;
            edge.cross(x - a).dot(self.normal) >= CLEAR * edge.length()
        })
    }
}
