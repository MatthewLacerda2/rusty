//! The curvature map (#1849): how sharply the surface turns near each texel, from
//! the mesh's own edges. Every edge two triangles share is a crease with a signed
//! strength: its dihedral angle over 90° (capped at 1), positive where the surface
//! folds away (convex), negative where it folds in (concave). A texel sums the
//! convex creases within [`RADIUS`] of its point, and apart the concave ones, each
//! fading linearly to nothing at the radius and each sum held to 1, and stores
//! `0.5 + 0.5 × (convex + concave)`: 0.5 flat, 1 on a right-angled convex edge,
//! 0 in a right-angled concave one. Summing makes the read follow the shape, not
//! the tessellation: a right angle reads the same whether it is one sharp edge, a
//! 45° + 45° chamfer or a rounded bevel of many small creases, as long as the
//! bevel is narrow beside the band. Seams in UV or normals don't break a crease:
//! corners are welded by position first. A texel reads only the creases of its own
//! connected surface, so a floor beside a separate crate gets no band from the
//! crate's edges; and two faces whose normals are opposite (a double-sided face's
//! outline) are no crease.

use std::collections::BTreeMap;
use std::f32::consts::FRAC_PI_2;

use glam::{Vec3, Vec4};

use super::source::Weld;
use super::{Baked, MeshSource};
use crate::core::collections::Map;
use crate::scene::lighting::lightmap::TexelPoint;

/// The band a crease lights, as a share of the mesh's bounds diagonal (at least
/// [`MIN_TEXELS`] texels wide, so a low resolution still shows it).
pub const RADIUS: f32 = 0.02;

/// The narrowest band, in texels.
const MIN_TEXELS: f32 = 3.0;

/// A dihedral angle below this (radians) is no crease: a quad's diagonal, float
/// noise on a flat face.
const MIN_ANGLE: f32 = 1e-3;

/// Two face normals whose dot product is below `-1 + OPPOSITE` face away from each
/// other: a face and its reversed copy, no crease.
const OPPOSITE: f32 = 1e-4;

/// One crease: its two ends, its signed strength in `-1..=1`, and the connected
/// surface it lies on.
struct Crease {
    a: Vec3,
    b: Vec3,
    strength: f32,
    surface: u32,
}

/// One face along an edge: its normal, its corner off the edge, its surface.
type Side = (Vec3, Vec3, u32);

/// The creases of one mesh, bucketed for the per-texel read.
pub(super) struct Curvature {
    creases: Vec<Crease>,
    grid: Grid,
    radius: f32,
    /// Per triangle, its connected surface.
    surfaces: Vec<u32>,
}

impl Curvature {
    pub(super) fn new(baked: &Baked) -> Self {
        let source = baked.source;
        let weld = source.weld();
        let surfaces = source.components(&weld);
        let creases = creases(source, &weld, &surfaces);
        let radius = (RADIUS * baked.diagonal).max(MIN_TEXELS * baked.texel_world());
        let grid = Grid::new(&creases, radius);
        Self {
            creases,
            grid,
            radius,
            surfaces,
        }
    }

    /// The curvature of the texel at `p`.
    pub(super) fn texel(&self, p: &TexelPoint) -> Vec4 {
        let surface = self.surfaces[p.tri() / 3];
        // A crease can be entered in several of the cubes read: each counts once,
        // summed in crease order so the bytes never depend on the grid's.
        let mut near: Vec<(u32, f32)> = Vec::new();
        for &k in self.grid.near(p.position) {
            let c = &self.creases[k as usize];
            if c.surface != surface {
                continue;
            }
            let d = segment_distance(p.position, c.a, c.b);
            if d < self.radius {
                near.push((k, c.strength * (1.0 - d / self.radius)));
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

/// Every edge exactly two triangles share whose faces turn by more than
/// [`MIN_ANGLE`] and are not [`OPPOSITE`], in a fixed order. A boundary edge or one
/// three faces share is skipped.
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

/// The distance from `p` to the segment `a`–`b`.
fn segment_distance(p: Vec3, a: Vec3, b: Vec3) -> f32 {
    let ab = b - a;
    let t = ((p - a).dot(ab) / ab.length_squared().max(f32::MIN_POSITIVE)).clamp(0.0, 1.0);
    p.distance(a + ab * t)
}

/// Creases bucketed in cubes `radius` wide, so a texel looks only at those near it.
/// A crease is entered in every cube a point sampled along it each half radius lands
/// in, so one within `radius` of a point is in a cube at most two away from it.
struct Grid {
    cell: f32,
    cells: Map<[i32; 3], Vec<u32>>,
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
        Self { cell, cells }
    }

    fn key(p: Vec3, cell: f32) -> [i32; 3] {
        (p / cell).floor().as_ivec3().to_array()
    }

    /// The creases entered within two cubes of `p` (a crease may come up more than
    /// once: the texel's read removes the duplicates).
    fn near(&self, p: Vec3) -> impl Iterator<Item = &u32> {
        let [x, y, z] = Self::key(p, self.cell);
        (-2..=2)
            .flat_map(move |dx| (-2..=2).flat_map(move |dy| (-2..=2).map(move |dz| [dx, dy, dz])))
            .filter_map(move |[dx, dy, dz]| self.cells.get(&[x + dx, y + dy, z + dz]))
            .flatten()
    }
}
