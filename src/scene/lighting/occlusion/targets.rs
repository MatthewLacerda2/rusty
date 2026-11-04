//! Where the occlusion bake aims its rays at an object (#1714): its extremes along
//! the eight box diagonals (a corner peeking past a door frame), and a lattice laid
//! on each of its triangles at most [`TARGET_SPACING`] apart, so the count grows
//! with the object's area. A gap that shows a patch of an object about that wide
//! is seen through, however large the object is.
//!
//! Each point keeps the plane of its triangle's drawn side, so a cell wholly
//! behind it skips it: no camera in that cell sees that side (a two-sided
//! triangle is never skipped).
//!
//! An object whose lattice would pass [`MAX_TARGETS`] points (some 2000 m² of
//! surface, a 600 m wall 3 m high) has no targets: the bake keeps it visible from
//! every cell. Conservative beats clever: a size the lattice could miss is never
//! culled.

use glam::Vec3;

use super::gather::Face;

/// The lattice's spacing on a triangle, metres: under a doorway's width, so a
/// doorway-sized patch always holds a point.
pub const TARGET_SPACING: f32 = 1.0;

/// The most target points one object gets; past it, it is never culled.
pub const MAX_TARGETS: usize = 4096;

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

/// `tris`' target points, `None` when it has more than [`MAX_TARGETS`].
pub(super) fn target_points(tris: &[([Vec3; 3], Face)]) -> Option<Vec<Target>> {
    let mut points = Vec::with_capacity(8);
    for k in 0..8 {
        let axis = |bit: i32| if k & bit == 0 { -1.0 } else { 1.0 };
        let diagonal = Vec3::new(axis(1), axis(2), axis(4));
        let best = tris.iter().flat_map(|t| t.0.iter().map(move |&p| (p, t)));
        let best = best.max_by(|a, b| a.0.dot(diagonal).total_cmp(&b.0.dot(diagonal)));
        if let Some((p, t)) = best {
            points.push(Target::on(p, t));
        }
    }
    let lattices: Vec<Lattice> = tris.iter().map(|t| Lattice::of(&t.0)).collect();
    let count = lattices.iter().map(Lattice::count);
    if points.len() + count.sum::<usize>() > MAX_TARGETS {
        return None;
    }
    for (t, lattice) in tris.iter().zip(&lattices) {
        lattice.points(|p| points.push(Target::on(p, t)));
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
            // A NaN or huge leg is clamped: its count then passes the cap anyway.
            n.clamp(1.0, MAX_TARGETS as f32) as u32
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

    fn count(&self) -> usize {
        let (n1, n2) = self.steps;
        let (n1, n2) = (u64::from(n1), u64::from(n2));
        // Row i holds j = 0..=floor(n2 (n1 - i) / n1).
        let row = |i: u64| (n2 * (n1 - i) / n1 + 1) as usize;
        (0..=n1).map(row).sum()
    }

    fn points(&self, mut out: impl FnMut(Vec3)) {
        let (n1, n2) = self.steps;
        for i in 0..=n1 {
            for j in (0..=n2).take_while(|&j| self.inside(i, j)) {
                let (u, v) = (i as f32 / n1 as f32, j as f32 / n2 as f32);
                out(self.apex + self.legs.0 * u + self.legs.1 * v);
            }
        }
    }
}
