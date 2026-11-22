use std::collections::BTreeMap;
use std::f32::consts::FRAC_PI_2;

use glam::{Vec3, Vec4};

use super::floor::{Radii, Raised};
use super::source::Weld;
use super::{Baked, MeshSource};
use crate::lightmap::TexelPoint;
use rusty_core::collections::Map;

/// A share of the bounds diagonal, when the bake names no `curvature_radius`.
pub const RADIUS: f32 = 0.02;

/// Radians: a smaller dihedral is no crease (a quad's diagonal, float noise).
const MIN_ANGLE: f32 = 1e-3;

/// Normals whose dot is under `-1 + OPPOSITE` are a face and its reversed copy: no crease.
const OPPOSITE: f32 = 1e-4;

struct Crease {
    a: Vec3,
    b: Vec3,
    strength: f32,
    surface: u32,
}

type Side = (Vec3, Vec3, u32);

pub(super) struct Curvature {
    creases: Vec<Crease>,
    grid: Grid,
    radii: Vec<f32>,
    surfaces: Vec<u32>,
}

impl Curvature {
    pub(super) fn new(baked: &Baked) -> (Self, Option<Raised>) {
        let source = baked.source;
        let weld = source.weld();
        let surfaces = source.components(&weld);
        let creases = creases(source, &weld, &surfaces);
        let radii = Radii::new(baked);
        let grid = Grid::new(&creases, radii.reach);
        let curvature = Self {
            creases,
            grid,
            radii: radii.per_tri,
            surfaces,
        };
        (curvature, radii.raised)
    }

    #[cfg(test)]
    pub(super) fn cell(&self) -> f32 {
        self.grid.cell
    }

    pub(super) fn texel(&self, p: &TexelPoint) -> Vec4 {
        let surface = self.surfaces[p.tri() / 3];
        let radius = self.radii[p.tri() / 3];
        let mut near: Vec<(u32, f32)> = Vec::new();
        for k in self.grid.near(p.position, radius) {
            let c = &self.creases[k as usize];
            if c.surface != surface {
                continue;
            }
            let d = segment_distance(p.position, c.a, c.b);
            if d < radius {
                near.push((k, c.strength * (1.0 - d / radius)));
            }
        }
        near.sort_unstable_by_key(|&(k, _)| k);
        near.dedup_by_key(|&mut (k, _)| k);
        let (mut convex, mut concave) = (0.0f32, 0.0f32);
        for (_, w) in near {
            if w > 0.0 {
                convex += w;
            } else {
                concave += w;
            }
        }
        Vec3::splat(0.5 + 0.5 * (convex.min(1.0) + concave.max(-1.0))).extend(1.0)
    }
}

fn creases(source: &MeshSource, weld: &Weld, surfaces: &[u32]) -> Vec<Crease> {
    let (ids, welded) = (&weld.ids, &weld.corners);
    let mut edges: BTreeMap<(u32, u32), Vec<Side>> = BTreeMap::new();
    for (k, t) in source.indices.chunks_exact(3).enumerate() {
        let [a, b, c] = [t[0], t[1], t[2]].map(|i| i as usize);
        let p = [a, b, c].map(|i| source.positions[i]);
        let Some(n) = source.face_normal(3 * k) else {
            continue;
        };
        for (i, j, far) in [(a, b, p[2]), (b, c, p[0]), (c, a, p[1])] {
            let key = (ids[i].min(ids[j]), ids[i].max(ids[j]));
            edges.entry(key).or_default().push((n, far, surfaces[k]));
        }
    }
    edges
        .into_iter()
        .filter_map(|((i, j), faces)| {
            let [(n1, _, surface), (n2, far2, _)] = faces[..] else {
                return None;
            };
            let cos = n1.dot(n2);
            let angle = cos.clamp(-1.0, 1.0).acos();
            let a = welded[i as usize];
            let sign = if (far2 - a).dot(n1) < 0.0 { 1.0 } else { -1.0 };
            (angle > MIN_ANGLE && cos > -1.0 + OPPOSITE).then(|| Crease {
                a,
                b: welded[j as usize],
                strength: sign * (angle / FRAC_PI_2).min(1.0),
                surface,
            })
        })
        .collect()
}

fn segment_distance(p: Vec3, a: Vec3, b: Vec3) -> f32 {
    let ab = b - a;
    let t = ((p - a).dot(ab) / ab.length_squared().max(f32::MIN_POSITIVE)).clamp(0.0, 1.0);
    p.distance(a + ab * t)
}

struct Grid {
    cell: f32,
    cells: Map<[i32; 3], Vec<u32>>,
    creases: u32,
}

impl Grid {
    fn new(creases: &[Crease], cell: f32) -> Self {
        let mut cells: Map<[i32; 3], Vec<u32>> = Map::default();
        for (k, c) in creases.iter().enumerate() {
            let steps = (c.a.distance(c.b) / (0.5 * cell)).ceil().min(1e6) as u32;
            for s in 0..=steps {
                let at = c.a.lerp(c.b, s as f32 / steps.max(1) as f32);
                let list = cells.entry(Self::key(at, cell)).or_default();
                if list.last() != Some(&(k as u32)) {
                    list.push(k as u32);
                }
            }
        }
        let creases = creases.len() as u32;
        Self {
            cell,
            cells,
            creases,
        }
    }

    fn key(p: Vec3, cell: f32) -> [i32; 3] {
        (p / cell).floor().as_ivec3().to_array()
    }

    fn near(&self, p: Vec3, radius: f32) -> Box<dyn Iterator<Item = u32> + '_> {
        // The `+ 1` ring holds while `new` samples each crease every half cell: change both
        // together.
        let n = ((radius / self.cell).ceil().min(1e3) as i32).saturating_add(1);
        let side = f64::from(2 * n + 1);
        if side * side * side >= f64::from(self.creases) {
            return Box::new(0..self.creases);
        }
        let [x, y, z] = Self::key(p, self.cell);
        Box::new(
            (-n..=n)
                .flat_map(move |dx| {
                    (-n..=n).flat_map(move |dy| (-n..=n).map(move |dz| [dx, dy, dz]))
                })
                .filter_map(move |[dx, dy, dz]| self.cells.get(&[x + dx, y + dy, z + dz]))
                .flatten()
                .copied(),
        )
    }
}
