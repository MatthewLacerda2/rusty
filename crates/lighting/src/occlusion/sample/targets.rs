use std::collections::btree_map::{BTreeMap, Entry};

use glam::Vec3;

use crate::occlusion::gather::Face;

/// Metres, under a doorway's width, so a doorway-sized patch always holds a point.
pub const TARGET_SPACING: f32 = 1.0;

/// Per object; an object past it is never culled.
pub const MAX_TARGETS: usize = 4096;

/// Metres, one kept point each: half the spacing, so a lattice on a wall keeps every point.
pub const VOXEL: f32 = TARGET_SPACING / 2.0;

/// A share of the way to the triangle's centroid, off the edge, so a neighbour sharing
/// that edge never blocks a ray to the point.
const INWARD: f32 = 0.01;

#[derive(Clone, Copy, Debug)]
pub(in crate::occlusion) struct Target {
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

    pub(super) fn drawn_toward(&self, point: Vec3) -> bool {
        self.front
            .is_none_or(|(n, d)| n.dot(point) >= d - 1e-4 * n.length())
    }

    pub(super) fn faces(&self, (min, max): (Vec3, Vec3)) -> bool {
        let Some((n, d)) = self.front else {
            return true;
        };
        let (centre, half) = ((min + max) * 0.5, (max - min) * 0.5);
        n.dot(centre) + n.abs().dot(half) >= d - 1e-4 * n.length()
    }
}

pub(in crate::occlusion) fn target_points(tris: &[([Vec3; 3], Face)]) -> Option<Vec<Target>> {
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
            n.max(1.0).min(MAX_TARGETS as f32) as u32
        };
        Self {
            apex: a,
            legs,
            steps: (steps(legs.0), steps(legs.1)),
        }
    }

    fn inside(&self, i: u32, j: u32) -> bool {
        let (n1, n2) = self.steps;
        u64::from(i) * u64::from(n2) + u64::from(j) * u64::from(n1) <= u64::from(n1) * u64::from(n2)
    }

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

const CHUNK: f32 = 2.0;

pub(super) struct Chunk {
    pub bounds: (Vec3, Vec3),
    pub targets: Vec<Target>,
}

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
