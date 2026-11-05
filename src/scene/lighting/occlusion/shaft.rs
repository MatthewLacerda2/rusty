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

use super::gather::Face;
use crate::scene::lighting::lightmap::{Rays, Tri};

/// How far, metres, both boxes must clear a face's plane, and a crossing its edges.
const CLEAR: f32 = 1e-3;

/// Every occluder triangle's convex face.
pub(super) struct Polygons {
    /// Per triangle (the BVH's order), its polygon.
    of: Vec<u32>,
    polygons: Vec<Polygon>,
}

/// A convex polygon, its corners wound about `normal` (unit), on the plane
/// `normal · p = offset`.
struct Polygon {
    corners: Vec<Vec3>,
    normal: Vec3,
    offset: f32,
}

impl Polygons {
    pub fn of(tris: &[Tri], faces: &[Face]) -> Self {
        let (mut of, mut polygons) = (Vec::with_capacity(tris.len()), Vec::<Polygon>::new());
        for (i, t) in tris.iter().enumerate() {
            let corners = [t.a, t.a + t.e1, t.a + t.e2];
            let joins = i > 0 && tris[i - 1].mesh == t.mesh && faces[i - 1] == faces[i];
            let last = polygons.last_mut().filter(|_| joins);
            if !last.is_some_and(|p| p.merge(corners)) {
                polygons.push(Polygon::new(corners));
            }
            of.push(polygons.len() as u32 - 1);
        }
        Self { of, polygons }
    }

    /// Whether the occluder face nearest along the line between the boxes' centres
    /// hides all of box `to` from all of box `from`. The face must be another
    /// object's than `object`'s and turned toward `from`; a point is a box whose
    /// corners meet.
    pub fn hides(
        &self,
        (rays, faces): (&Rays, &[Face]),
        from: (Vec3, Vec3),
        to: (Vec3, Vec3),
        object: u32,
    ) -> bool {
        let (c, t) = ((from.0 + from.1) * 0.5, (to.0 + to.1) * 0.5);
        let delta = t - c;
        let reach = delta.length();
        if reach.is_nan() || reach <= CLEAR {
            return false;
        }
        let dir = delta / reach;
        let Some(hit) = rays.closest(c, dir, 0.0, reach) else {
            return false;
        };
        let tri = &rays.tris[hit.tri];
        if tri.mesh == object || !faces[hit.tri].stops(tri.normal(), dir) {
            return false;
        }
        self.polygons[self.of[hit.tri] as usize].separates(from, to)
    }
}

/// A box's corners, one when they meet.
fn corners((min, max): (Vec3, Vec3)) -> Vec<Vec3> {
    if min == max {
        return vec![min];
    }
    let pick = |k: usize, bit: usize, axis: usize| if k & bit == 0 { min[axis] } else { max[axis] };
    (0..8)
        .map(|k| Vec3::new(pick(k, 1, 0), pick(k, 2, 1), pick(k, 4, 2)))
        .collect()
}

impl Polygon {
    fn new(corners: [Vec3; 3]) -> Self {
        let normal = (corners[1] - corners[0])
            .cross(corners[2] - corners[0])
            .normalize_or_zero();
        Self {
            corners: corners.to_vec(),
            normal,
            offset: normal.dot(corners[0]),
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
        let (near, far) = (corners(from), corners(to));
        let sign = side((from.0 + from.1) * 0.5).signum();
        let clear = near.iter().all(|&p| side(p) * sign > CLEAR)
            && far.iter().all(|&p| side(p) * sign < -CLEAR);
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
