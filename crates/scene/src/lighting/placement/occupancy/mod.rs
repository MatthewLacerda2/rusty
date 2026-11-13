//! A 3D occupancy grid of the static scene (#1737): which cells of the level are
//! solid, and how far every open cell is from the nearest solid one.
//!
//! The floor plan (`floor.rs`) says where an actor stands; this says what is
//! around and above it. Placement reads it to tell an enclosed room from open air
//! (rays cast up and sideways), to measure how cluttered a room is, to keep a
//! capture point clear of geometry, and to place light probes in the air actors
//! move through but never inside a wall or above a ceiling.
//!
//! Built from [`Solid`]s: the static colliders' world triangles, handed over as
//! data by the bake (physics tessellates them, keeping the arrow physics → scene),
//! or, with none handed over, the colliders' world boxes. A convex solid is filled
//! between its lowest and highest surface in each column; a mesh collider marks
//! only its surface. Clearance is a search of the cells around a point, out to
//! `REACH`: every question placement asks is about the space near a probe, so
//! no distance field over the whole level is kept.
//!
//! Memory is one bit a cell. A Dust2-sized level (≈ 200 × 200 × 40 m) takes
//! 12.8 M cells at `CELL` = 0.5 m (≈ 1.6 MB); a level past `MAX_CELLS`
//! (≈ 32 MB) is gridded coarser.
//!
//! Pure data + math — deterministic, scan order, no RNG/clock.

mod voxelize;

use glam::Vec3;

pub use self::voxelize::Solid;
use crate::Scene;

/// The occupancy grid's cell edge, world units.
pub const CELL: f32 = 0.5;
/// The most cells a grid holds (≈ 32 MB of bits); a larger level is gridded
/// coarser.
pub const MAX_CELLS: usize = 256_000_000;
/// The farthest [`Occupancy::clearance`] looks, world units.
pub const REACH: f32 = 2.0;

/// The static scene as a grid of solid and open cells, x fastest, then y, then z.
#[derive(Clone, Debug)]
pub struct Occupancy {
    /// The min corner of cell `(0, 0, 0)`.
    pub origin: Vec3,
    pub cell: f32,
    pub dims: [usize; 3],
    /// One bit per cell, set where it is solid.
    solid: Vec<u64>,
}

impl Occupancy {
    /// The grid of `solids` at `cell` (coarser past `MAX_CELLS`), one open cell
    /// of margin around them. `None` with nothing solid.
    pub fn build(solids: &[Solid], cell: f32) -> Option<Self> {
        Self::build_capped(solids, cell, MAX_CELLS)
    }

    /// [`Self::build`] under a cap of `max_cells`.
    fn build_capped(solids: &[Solid], cell: f32, max_cells: usize) -> Option<Self> {
        let (lo, hi) = (solids.iter().filter_map(Solid::bounds))
            .reduce(|a, b| (a.0.min(b.0), a.1.max(b.1)))?;
        if !(hi - lo).is_finite() {
            return None; // corners so far apart their span overflows
        }
        let mut cell = cell.max(0.05);
        let dims = |cell: f32| {
            ((hi - lo) / cell)
                .ceil()
                .as_uvec3()
                .to_array()
                .map(|n| n as usize + 2)
        };
        let count = |d: [usize; 3]| d.iter().fold(1usize, |n, &d| n.saturating_mul(d));
        while count(dims(cell)) > max_cells {
            cell *= 1.25;
        }
        let mut grid = Self {
            origin: lo - Vec3::splat(cell),
            cell,
            dims: dims(cell),
            solid: Vec::new(),
        };
        grid.solid = voxelize::mark(&grid, solids);
        Some(grid)
    }

    /// The grid of `scene`'s static, active, solid colliders by their world boxes:
    /// what placement uses when the bake hands over no triangles.
    pub fn from_static_boxes(scene: &Scene, cell: f32) -> Option<Self> {
        let solids: Vec<Solid> = (super::floor::static_boxes(scene).into_iter())
            .map(|(lo, hi)| Solid::cuboid(lo, hi))
            .collect();
        Self::build(&solids, cell)
    }

    /// The bytes the grid holds.
    pub fn memory_bytes(&self) -> usize {
        self.solid.len() * 8
    }

    /// The cell holding `p`, if it is on the grid.
    fn cell_of(&self, p: Vec3) -> Option<[usize; 3]> {
        let c = ((p - self.origin) / self.cell).floor();
        let [nx, ny, nz] = self.dims;
        let inside = c.cmpge(Vec3::ZERO).all()
            && (c.x as usize) < nx
            && (c.y as usize) < ny
            && (c.z as usize) < nz;
        inside.then_some([c.x as usize, c.y as usize, c.z as usize])
    }

    /// Whether cell `[x, y, z]` is solid; off the grid is open.
    fn solid_cell(&self, [x, y, z]: [i64; 3]) -> bool {
        let [nx, ny, nz] = self.dims.map(|n| n as i64);
        if x < 0 || y < 0 || z < 0 || x >= nx || y >= ny || z >= nz {
            return false;
        }
        let i = ((z * ny + y) * nx + x) as usize;
        self.solid[i / 64] >> (i % 64) & 1 == 1
    }

    /// Whether `p` is inside solid geometry. Off the grid is open.
    pub fn is_solid(&self, p: Vec3) -> bool {
        self.cell_of(p)
            .is_some_and(|c| self.solid_cell(c.map(|v| v as i64)))
    }

    /// About how far `p` is from the nearest solid, world units, to a cell: 0
    /// inside one, at most `REACH`. Searches the cells around `p` shell by
    /// shell, nearest first.
    pub fn clearance(&self, p: Vec3) -> f32 {
        let q = ((p - self.origin) / self.cell).floor();
        let c = [q.x as i64, q.y as i64, q.z as i64];
        if self.solid_cell(c) {
            return 0.0;
        }
        let reach = (REACH / self.cell).ceil() as i64;
        let mut best = f32::MAX;
        for r in 1..=reach {
            if best <= r as f32 {
                break; // every cell of this shell is farther than the best
            }
            for dz in -r..=r {
                for dy in -r..=r {
                    for dx in -r..=r {
                        if dx.abs().max(dy.abs()).max(dz.abs()) != r {
                            continue;
                        }
                        if self.solid_cell([c[0] + dx, c[1] + dy, c[2] + dz]) {
                            let d = ((dx * dx + dy * dy + dz * dz) as f32).sqrt();
                            best = best.min(d);
                        }
                    }
                }
            }
        }
        // Centre to centre, less half a cell: the gap to the solid's face.
        ((best - 0.5) * self.cell).clamp(0.0, REACH)
    }

    /// How far a ray from `from` along `dir` (normalized here) goes before it
    /// enters solid geometry, `None` when it leaves the grid or passes `max` first.
    /// A ray starting off the grid starts where it enters it. Marches in steps of
    /// half a cell, then finds the solid's face to a hundredth of a cell.
    pub fn ray(&self, from: Vec3, dir: Vec3, max: f32) -> Option<f32> {
        let dir = dir.normalize_or_zero();
        if dir == Vec3::ZERO {
            return None;
        }
        let (enter, leave) = self.span_of(from, dir)?;
        let (mut t, max) = ((enter + 1.0e-4).max(0.0), leave.min(max));
        let mut open = t;
        while t <= max {
            let p = from + dir * t;
            if self.is_solid(p) {
                // The face lies between the last open sample and this one.
                let (mut a, mut b) = (open, t);
                while b - a > self.cell * 0.01 {
                    let m = (a + b) * 0.5;
                    (a, b) = if self.is_solid(from + dir * m) {
                        (a, m)
                    } else {
                        (m, b)
                    };
                }
                return Some(b);
            }
            open = t;
            t += self.cell * 0.5;
        }
        None
    }

    /// Where a ray from `from` along unit `dir` is inside the grid's box, as
    /// distances along it; `None` when it misses the box.
    fn span_of(&self, from: Vec3, dir: Vec3) -> Option<(f32, f32)> {
        let lo = self.origin;
        let hi = lo
            + Vec3::new(
                self.dims[0] as f32,
                self.dims[1] as f32,
                self.dims[2] as f32,
            ) * self.cell;
        let (mut enter, mut leave) = (f32::MIN, f32::MAX);
        for axis in 0..3 {
            let (o, d) = (from[axis], dir[axis]);
            if d.abs() < 1.0e-9 {
                if o < lo[axis] || o >= hi[axis] {
                    return None;
                }
                continue;
            }
            let (a, b) = ((lo[axis] - o) / d, (hi[axis] - o) / d);
            (enter, leave) = (enter.max(a.min(b)), leave.min(a.max(b)));
        }
        (leave >= enter.max(0.0)).then_some((enter, leave - 1.0e-4))
    }

    /// The fraction of `[lo, hi]`'s cells that are solid; 0 for an empty box.
    pub fn solid_fraction(&self, lo: Vec3, hi: Vec3) -> f32 {
        let (mut solid, mut all) = (0usize, 0usize);
        let half = self.cell * 0.5;
        let mut z = lo.z + half;
        while z < hi.z {
            let mut y = lo.y + half;
            while y < hi.y {
                let mut x = lo.x + half;
                while x < hi.x {
                    all += 1;
                    solid += usize::from(self.is_solid(Vec3::new(x, y, z)));
                    x += self.cell;
                }
                y += self.cell;
            }
            z += self.cell;
        }
        if all == 0 {
            return 0.0;
        }
        solid as f32 / all as f32
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
