//! Where the occlusion bake aims its rays at an object (#1714): its extremes along
//! the eight box diagonals (a corner peeking past a door frame), and a lattice laid
//! on each of its triangles at most [`TARGET_SPACING`] apart, kept at one point per
//! [`VOXEL`] cube of space, so the count follows the object's surface area, not its
//! triangle count: a dense 1.5 m prop gets about the points a box of its size does.
//! A gap that shows a patch of an object about [`TARGET_SPACING`] wide is seen
//! through, however large the object is.
//!
//! Each point keeps the plane of its triangle's drawn side, so a cell wholly
//! behind it skips it, and a ray from a cell point behind it is never cast: no
//! camera there sees that side. A two-sided triangle, or a voxel whose points lie on
//! planes that disagree (a thin prop's two faces, a curved surface), is never skipped.
//!
//! An object whose points would pass [`MAX_TARGETS`] (some 4000 m² of surface, a
//! 600 m wall 3 m high), or that has a vertex that is not finite, has no targets:
//! the bake keeps it visible from every cell. Conservative beats clever: a size the
//! lattice could miss is never culled.

use std::collections::btree_map::{BTreeMap, Entry};

use glam::Vec3;

use super::gather::Face;

/// The lattice's spacing on a triangle, metres: under a doorway's width, so a
/// doorway-sized patch always holds a point.
pub const TARGET_SPACING: f32 = 1.0;

/// The most target points one object gets; past it, it is never culled.
pub const MAX_TARGETS: usize = 4096;

/// The edge of the cubes of space that keep one target point each, metres: half
/// the spacing, so a lattice on a wall keeps every point, and a fine mesh's corners
/// fold into about one point per quarter square metre.
pub const VOXEL: f32 = TARGET_SPACING / 2.0;

/// How far a point is pulled toward its triangle's centre, so it lies inside it
/// and a ray to it is not stopped by a neighbour sharing its edge.
const INWARD: f32 = 0.01;

/// One point a ray is aimed at, and the plane of its triangle's drawn side
/// (normal and offset; `None` when both sides draw).
#[derive(Clone, Copy, Debug)]
pub(super) struct Target {
    pub at: Vec3,
    front: Option<(Vec3, f32)>,
}

impl Target {
    fn on(p: Vec3, (t, face): &([Vec3; 3], Face)) -> Self {
        let front = face.front((t[1] - t[0]).cross(t[2] - t[0]));
        Self {
            at: p.lerp((t[0] + t[1] + t[2]) / 3.0, INWARD),
            front: front.map(|n| (n, n.dot(t[0]))),
        }
    }

    /// Fold `other`, a point in the same voxel, into this one: it keeps its plane
    /// only when `other` lies on it and faces the same way, else it is seen from
    /// both sides.
    fn absorb(&mut self, other: &Target) {
        let Some((n, d)) = self.front else {
            return;
        };
        let same = other.front.is_some_and(|(m, _)| {
            n.normalize_or_zero().dot(m.normalize_or_zero()) > 0.999
                && (n.dot(other.at) - d).abs() <= 0.02 * n.length()
        });
        if !same {
            self.front = None;
        }
    }

    /// Whether `point` is on its drawn side: a ray from behind reaches only its
    /// undrawn back.
    pub(super) fn drawn_toward(&self, point: Vec3) -> bool {
        self.front
            .is_none_or(|(n, d)| n.dot(point) >= d - 1e-4 * n.length())
    }

    /// Whether any point of the box `(min, max)` is on its drawn side.
    pub(super) fn faces(&self, (min, max): (Vec3, Vec3)) -> bool {
        let Some((n, d)) = self.front else {
            return true;
        };
        let (centre, half) = ((min + max) * 0.5, (max - min) * 0.5);
        // The box's farthest reach along n, against the plane, with a margin.
        n.dot(centre) + n.abs().dot(half) >= d - 1e-4 * n.length()
    }
}

/// `tris`' target points, `None` when it has more than [`MAX_TARGETS`] or a corner
/// that is not finite.
pub(super) fn target_points(tris: &[([Vec3; 3], Face)]) -> Option<Vec<Target>> {
    if !tris.iter().all(|t| t.0.iter().all(|p| p.is_finite())) {
        return None;
    }
    let mut points = Vec::with_capacity(64);
    for k in 0..8 {
        let axis = |bit: i32| if k & bit == 0 { -1.0 } else { 1.0 };
        let diagonal = Vec3::new(axis(1), axis(2), axis(4));
        let best = tris.iter().flat_map(|t| t.0.iter().map(move |&p| (p, t)));
        let best = best.max_by(|a, b| a.0.dot(diagonal).total_cmp(&b.0.dot(diagonal)));
        if let Some((p, t)) = best {
            points.push(Target::on(p, t));
        }
    }
    let mut voxels: BTreeMap<[i32; 3], usize> = BTreeMap::new();
    for t in tris {
        let fits = Lattice::of(&t.0).points(|p| {
            let target = Target::on(p, t);
            match voxels.entry((target.at / VOXEL).floor().as_ivec3().to_array()) {
                Entry::Vacant(slot) => {
                    slot.insert(points.len());
                    points.push(target);
                }
                Entry::Occupied(kept) => points[*kept.get()].absorb(&target),
            }
            points.len() <= MAX_TARGETS
        });
        if !fits {
            return None;
        }
    }
    Some(points)
}

/// One triangle's lattice: from the corner opposite its longest edge (the widest
/// angle, so a lattice cell's diagonal stays under twice the spacing), `n1` steps
/// along one leg and `n2` along the other, keeping the points inside.
struct Lattice {
    apex: Vec3,
    legs: (Vec3, Vec3),
    steps: (u32, u32),
}

impl Lattice {
    fn of(t: &[Vec3; 3]) -> Self {
        let edge = |i: usize| (t[(i + 2) % 3] - t[(i + 1) % 3]).length_squared();
        let apex = (0..3)
            .max_by(|&a, &b| edge(a).total_cmp(&edge(b)))
            .unwrap_or(0);
        let (a, b, c) = (t[apex], t[(apex + 1) % 3], t[(apex + 2) % 3]);
        let legs = (b - a, c - a);
        let steps = |leg: Vec3| {
            let n = (leg.length() / TARGET_SPACING).ceil();
            // A huge leg is clamped (its points then pass the cap anyway); `max`
            // takes 1 over a NaN, so a step count is never 0.
            n.max(1.0).min(MAX_TARGETS as f32) as u32
        };
        Self {
            apex: a,
            legs,
            steps: (steps(legs.0), steps(legs.1)),
        }
    }

    /// Whether lattice point `(i, j)` lies on the triangle.
    fn inside(&self, i: u32, j: u32) -> bool {
        let (n1, n2) = self.steps;
        // In integers: i/n1 + j/n2 <= 1.
        u64::from(i) * u64::from(n2) + u64::from(j) * u64::from(n1) <= u64::from(n1) * u64::from(n2)
    }

    /// Hand each point to `out` while it returns true; whether it always did.
    fn points(&self, mut out: impl FnMut(Vec3) -> bool) -> bool {
        let (n1, n2) = self.steps;
        for i in 0..=n1 {
            for j in (0..=n2).take_while(|&j| self.inside(i, j)) {
                let (u, v) = (i as f32 / n1 as f32, j as f32 / n2 as f32);
                if !out(self.apex + self.legs.0 * u + self.legs.1 * v) {
                    return false;
                }
            }
        }
        true
    }
}

/// The edge of the cubes of space an object's targets are grouped by, metres:
/// one polygon that hides a chunk's box spares every ray to its targets.
const CHUNK: f32 = 2.0;

/// Some of an object's targets, near one another, and the box round them.
pub(super) struct Chunk {
    pub bounds: (Vec3, Vec3),
    pub targets: Vec<Target>,
}

/// `points` grouped by the [`CHUNK`] cube each lies in, in the cubes' order.
pub(super) fn chunks(points: Vec<Target>) -> Vec<Chunk> {
    let mut cubes: BTreeMap<[i32; 3], Vec<Target>> = BTreeMap::new();
    for t in points {
        let key = (t.at / CHUNK).floor().as_ivec3().to_array();
        cubes.entry(key).or_default().push(t);
    }
    let chunk = |targets: Vec<Target>| {
        let at = targets.iter().map(|t| t.at);
        let lo = at.clone().reduce(Vec3::min).unwrap_or_default();
        let hi = at.reduce(Vec3::max).unwrap_or_default();
        Chunk {
            bounds: (lo, hi),
            targets,
        }
    };
    cubes.into_values().map(chunk).collect()
}
