//! The occlusion bake's shaft test (#1714): whether one occluder's flat face hides
//! every point of one box from every point of another, so no ray between them
//! need be cast. Exact and one way: it says "hidden" only when every segment from
//! one box to the other crosses the face, else the bake casts its sampled rays.
//!
//! The segments' crossings of a plane lie in the convex hull of the crossings of
//! the segments between the two boxes' corners (a central projection keeps convex
//! sets convex), so a convex face holding those crossings holds them all. A face
//! is a triangle, or two of one mesh's triangles that share an edge and make a
//! convex quad on one plane (a box's side), merged as they come.

use glam::Vec3;

use super::sheets::{sheets, Member, Sheet};
use super::{corners, opposite, CLEAR};
use crate::lighting::lightmap::{Rays, Tri};
use crate::lighting::occlusion::gather::Face;

/// Every occluder triangle's convex face.
pub(super) struct Polygons {
    /// Per triangle (the BVH's order), its polygon.
    of: Vec<u32>,
    polygons: Vec<Polygon>,
    /// The planes several polygons share, fused, and per polygon its sheet
    /// (`u32::MAX` when it has none).
    sheets: Vec<Sheet>,
    sheet_of: Vec<u32>,
}

/// A convex polygon, its corners wound about `normal` (unit), on the plane
/// `normal · p = offset`; the object it belongs to and the side it draws.
struct Polygon {
    corners: Vec<Vec3>,
    normal: Vec3,
    offset: f32,
    mesh: u32,
    face: Face,
}

impl Polygons {
    /// The polygons of `tris`, whose corners are `corners` as gathered: not
    /// rebuilt from a `Tri`'s edges, whose float sum can lose a shared vertex's
    /// low bits, so two faces sharing a corner share it bit for bit (`sheets`).
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

    /// The occluder polygon nearest along the line between the boxes' centres,
    /// when it hides all of box `to` from all of box `from` ([`Self::hides`]).
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

    /// Whether polygon `polygon` hides all of box `to` from all of box `from`:
    /// another object's than `object`'s, its drawn side turned toward `from`, and
    /// every segment between the boxes crossing it. A point is a box whose
    /// corners meet.
    pub fn hides(&self, polygon: u32, from: (Vec3, Vec3), to: (Vec3, Vec3), object: u32) -> bool {
        let sheet = self.sheets.get(self.sheet_of[polygon as usize] as usize);
        if sheet.is_some_and(|s| !s.holds(object) && s.hides(from, to)) {
            return true;
        }
        let p = &self.polygons[polygon as usize];
        // Boxes on opposite sides of its plane see it from one side, so the
        // centres' direction says which.
        let dir = (to.0 + to.1) * 0.5 - (from.0 + from.1) * 0.5;
        p.mesh != object && p.face.stops(p.normal, dir) && p.separates(from, to)
    }

    /// How many sheets the polygons fused into, and the bytes they take.
    #[cfg(all(test, feature = "dev"))]
    pub fn sheet_bytes(&self) -> (usize, usize) {
        (
            self.sheets.len(),
            self.sheets.iter().map(Sheet::bytes).sum(),
        )
    }

    /// The polygon triangle `tri` (the BVH's order) belongs to.
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

    /// Take in the triangle `t` when it shares an edge with this triangle and the
    /// two make a convex quad on one plane, wound the same way.
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
            // Wound the same way, `t` runs the shared edge backwards: q then p.
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

    /// Whether every segment from box `from` to box `to` crosses this polygon: the
    /// boxes clear its plane on opposite sides, and every corner-to-corner segment
    /// crosses it inside its edges.
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

    /// Whether `x`, on the plane, lies inside the polygon by a margin.
    fn holds(&self, x: Vec3) -> bool {
        let k = self.corners.len();
        (0..k).all(|i| {
            let (a, b) = (self.corners[i], self.corners[(i + 1) % k]);
            let edge = b - a;
            edge.cross(x - a).dot(self.normal) >= CLEAR * edge.length()
        })
    }
}
