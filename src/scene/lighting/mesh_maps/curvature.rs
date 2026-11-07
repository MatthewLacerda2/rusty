//! The curvature map (#1849): how sharply the surface turns near each texel, from
//! the mesh's own edges. Every edge two triangles share is a crease with a signed
//! strength: its dihedral angle over 90° (capped at 1), positive where the surface
//! folds away (convex), negative where it folds in (concave). A texel reads the
//! strongest convex and the strongest concave crease within [`RADIUS`] of its
//! point, each fading linearly to nothing at the radius, and stores
//! `0.5 + 0.5 × (convex + concave)`: 0.5 flat, 1 on a right-angled convex edge,
//! 0 in a right-angled concave one. Seams in UV or normals don't break a crease:
//! corners are welded by position first.

use std::collections::BTreeMap;
use std::f32::consts::FRAC_PI_2;

use glam::{Vec3, Vec4};

use super::{per_texel, Baked, MeshSource};
use crate::core::collections::Map;

/// The band a crease lights, as a share of the mesh's bounds diagonal (at least
/// [`MIN_TEXELS`] texels wide, so a low resolution still shows it).
pub const RADIUS: f32 = 0.02;

/// The narrowest band, in texels.
const MIN_TEXELS: f32 = 3.0;

/// A dihedral angle below this (radians) is no crease: a quad's diagonal, float
/// noise on a flat face.
const MIN_ANGLE: f32 = 1e-3;

/// One crease: its two ends and its signed strength in `-1..=1`.
struct Crease {
    a: Vec3,
    b: Vec3,
    strength: f32,
}

/// The curvature of every covered texel, in point order.
pub(super) fn bake(baked: &Baked) -> Vec<Vec4> {
    let creases = creases(baked.source);
    let radius = (RADIUS * baked.diagonal).max(MIN_TEXELS * baked.texel_world());
    let grid = Grid::new(&creases, radius);
    per_texel(&baked.points, |_, p| {
        let (mut convex, mut concave) = (0.0f32, 0.0f32);
        for &k in grid.near(p.position) {
            let c = &creases[k as usize];
            let d = segment_distance(p.position, c.a, c.b);
            if d < radius {
                let w = c.strength * (1.0 - d / radius);
                (convex, concave) = (convex.max(w), concave.min(w));
            }
        }
        Vec3::splat(0.5 + 0.5 * (convex + concave).clamp(-1.0, 1.0)).extend(1.0)
    })
}

/// Every edge exactly two triangles share whose faces turn by more than
/// [`MIN_ANGLE`], in a fixed order. A boundary edge or one three faces share is
/// skipped.
fn creases(source: &MeshSource) -> Vec<Crease> {
    let mut weld: Map<[u32; 3], u32> = Map::default();
    let ids: Vec<u32> = source
        .positions
        .iter()
        .map(|p| {
            let next = weld.len() as u32;
            *weld.entry(p.to_array().map(f32::to_bits)).or_insert(next)
        })
        .collect();
    // (welded end, welded end) → (face normal, the face's third corner).
    let mut edges: BTreeMap<(u32, u32), Vec<(Vec3, Vec3)>> = BTreeMap::new();
    for t in source.indices.chunks_exact(3) {
        let [a, b, c] = [t[0], t[1], t[2]].map(|i| i as usize);
        let p = [a, b, c].map(|i| source.positions[i]);
        let Some(n) = (p[1] - p[0]).cross(p[2] - p[0]).try_normalize() else {
            continue;
        };
        for (i, j, far) in [(a, b, p[2]), (b, c, p[0]), (c, a, p[1])] {
            let key = (ids[i].min(ids[j]), ids[i].max(ids[j]));
            edges.entry(key).or_default().push((n, far));
        }
    }
    let welded: Vec<Vec3> = {
        let mut out = vec![Vec3::ZERO; weld.len()];
        for (p, &id) in source.positions.iter().zip(&ids) {
            out[id as usize] = *p;
        }
        out
    };
    edges
        .into_iter()
        .filter_map(|((i, j), faces)| {
            let [(n1, _), (n2, far2)] = faces[..] else {
                return None;
            };
            let angle = n1.dot(n2).clamp(-1.0, 1.0).acos();
            let a = welded[i as usize];
            let sign = if (far2 - a).dot(n1) < 0.0 { 1.0 } else { -1.0 };
            (angle > MIN_ANGLE).then(|| Crease {
                a,
                b: welded[j as usize],
                strength: sign * (angle / FRAC_PI_2).min(1.0),
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
    /// once; the texel's strongest-wins read doesn't mind).
    fn near(&self, p: Vec3) -> impl Iterator<Item = &u32> {
        let [x, y, z] = Self::key(p, self.cell);
        (-2..=2)
            .flat_map(move |dx| (-2..=2).flat_map(move |dy| (-2..=2).map(move |dz| [dx, dy, dz])))
            .filter_map(move |[dx, dy, dz]| self.cells.get(&[x + dx, y + dy, z + dz]))
            .flatten()
    }
}
