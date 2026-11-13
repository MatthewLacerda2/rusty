//! Marking the solid cells (#1737): each triangle's surface, sampled finer than a
//! cell, and for a convex solid every cell between its lowest and highest surface
//! cell in each column (the navmesh bake's rule for convex shapes).

use glam::Vec3;

use super::Occupancy;

/// How far, in cells, a sample leans toward its solid's middle.
const LEAN: f32 = 1.0e-3;
/// The most samples along a triangle's edge: a 2 km edge at a 0.5 m cell.
const MAX_SAMPLES: usize = 8192;

/// One static solid as world triangles, as `physics::shape::triangles` gives them.
#[derive(Clone, Debug, PartialEq)]
pub struct Solid {
    pub vertices: Vec<Vec3>,
    pub triangles: Vec<[u32; 3]>,
    /// A vertical line crosses it in one interval (everything but a non-convex
    /// mesh collider), so its inside is filled, not only its surface.
    pub convex: bool,
}

impl Solid {
    /// The axis-aligned box `[lo, hi]`.
    pub fn cuboid(lo: Vec3, hi: Vec3) -> Self {
        let vertices = (0..8)
            .map(|i| {
                let pick = |bit: u32, a: f32, b: f32| if i & bit == 0 { a } else { b };
                Vec3::new(
                    pick(1, lo.x, hi.x),
                    pick(2, lo.y, hi.y),
                    pick(4, lo.z, hi.z),
                )
            })
            .collect();
        // Two triangles per face; winding is not read.
        let triangles = vec![
            [0, 1, 3],
            [0, 3, 2],
            [4, 5, 7],
            [4, 7, 6], // z faces
            [0, 1, 5],
            [0, 5, 4],
            [2, 3, 7],
            [2, 7, 6], // y faces
            [0, 2, 6],
            [0, 6, 4],
            [1, 3, 7],
            [1, 7, 5], // x faces
        ];
        Self {
            vertices,
            triangles,
            convex: true,
        }
    }

    /// Its world bounds, `None` without a triangle.
    pub fn bounds(&self) -> Option<(Vec3, Vec3)> {
        (self.triangles.iter().flatten())
            .filter_map(|&i| self.vertices.get(i as usize).copied())
            .filter(|v| v.is_finite())
            .map(|v| (v, v))
            .reduce(|a, b| (a.0.min(b.0), a.1.max(b.1)))
    }
}

/// One bit per cell of `grid`, set where a solid fills it.
pub(super) fn mark(grid: &Occupancy, solids: &[Solid]) -> Vec<u64> {
    let [nx, ny, _] = grid.dims;
    let mut solid = Bits(vec![0; grid.dims.iter().product::<usize>().div_ceil(64)]);
    for s in solids {
        let Some((lo, hi)) = s.bounds() else { continue };
        // A convex solid's lowest and highest cell in each column of its footprint.
        let (x0, z0) = (column(grid, lo.x, 0), column(grid, lo.z, 2));
        let (w, d) = (
            column(grid, hi.x, 0) + 1 - x0,
            column(grid, hi.z, 2) + 1 - z0,
        );
        let mut span = vec![(usize::MAX, 0usize); if s.convex { w * d } else { 0 }];
        // Samples lean a hair toward the solid's middle, so a face on a cell
        // boundary marks the cell inside it, not the one outside.
        let middle = (lo + hi) * 0.5;
        for t in &s.triangles {
            let Some(tri) = triangle(s, t) else { continue };
            for_each_cell(grid, tri, middle, |x, y, z| {
                solid.set((z * ny + y) * nx + x);
                let at = (x.checked_sub(x0).filter(|&dx| dx < w))
                    .zip(z.checked_sub(z0))
                    .and_then(|(dx, dz)| span.get_mut(dz * w + dx));
                if let Some(c) = at {
                    *c = (c.0.min(y), c.1.max(y));
                }
            });
        }
        for (i, &(y0, y1)) in span.iter().enumerate() {
            let (x, z) = (x0 + i % w, z0 + i / w);
            for y in y0..=y1.min(ny - 1) {
                solid.set((z * ny + y) * nx + x);
            }
        }
    }
    solid.0
}

/// A bit per cell.
struct Bits(Vec<u64>);

impl Bits {
    fn set(&mut self, i: usize) {
        self.0[i / 64] |= 1 << (i % 64);
    }
}

/// The cell along `axis` holding world coordinate `v`, held to the grid.
fn column(grid: &Occupancy, v: f32, axis: usize) -> usize {
    let c = ((v - grid.origin[axis]) / grid.cell).floor().max(0.0) as usize;
    c.min(grid.dims[axis] - 1)
}

/// Triangle `t`'s corners, `None` when one is missing or not finite.
fn triangle(s: &Solid, t: &[u32; 3]) -> Option<[Vec3; 3]> {
    let v = t.map(|i| s.vertices.get(i as usize).copied());
    let [a, b, c] = [v[0]?, v[1]?, v[2]?];
    (a.is_finite() && b.is_finite() && c.is_finite()).then_some([a, b, c])
}

/// Call `f` with every cell the triangle passes through, sampling it at under half
/// a cell (a cell may be called more than once).
fn for_each_cell(
    grid: &Occupancy,
    [a, b, c]: [Vec3; 3],
    middle: Vec3,
    mut f: impl FnMut(usize, usize, usize),
) {
    let longest = a.distance(b).max(b.distance(c)).max(c.distance(a));
    let n = ((longest / (grid.cell * 0.5)).ceil() as usize).clamp(1, MAX_SAMPLES);
    let [nx, ny, nz] = grid.dims;
    for i in 0..=n {
        for j in 0..=(n - i) {
            let (u, v) = (i as f32 / n as f32, j as f32 / n as f32);
            let p = a + (b - a) * u + (c - a) * v;
            let p = p + (middle - p).signum() * grid.cell * LEAN;
            let q = ((p - grid.origin) / grid.cell).floor();
            if q.cmplt(Vec3::ZERO).any() {
                continue;
            }
            let (x, y, z) = (q.x as usize, q.y as usize, q.z as usize);
            if x < nx && y < ny && z < nz {
                f(x, y, z);
            }
        }
    }
}
