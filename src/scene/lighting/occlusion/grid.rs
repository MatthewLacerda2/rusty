//! The PVS's cells (#1714): a uniform 3D grid of cubes over the static bounds, one
//! cell of margin on every side, so a camera standing at a wall's outer face is
//! still in a cell. A level too big for [`MAX_CELLS`] cells of [`CELL_SIZE`] is cut
//! into fewer, larger ones.

use glam::{UVec3, Vec3};

/// A cell's edge, metres, on a level small enough to take it: about a doorway's
/// width, so a cell rarely sees both sides of a wall it straddles.
pub const CELL_SIZE: f32 = 2.0;

/// The most cells a grid holds: a Kolvik-sized interior (52 × 42 × 6 m) takes 2 m
/// cells, a Dust2-sized level larger ones. It bounds the bake's work (cells times
/// objects) and the file.
pub const MAX_CELLS: usize = 16_384;

/// The cells: cube `(x, y, z)` spans `origin + (x, y, z) * cell` to one `cell` more.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Grid {
    pub origin: Vec3,
    pub cell: f32,
    pub dims: UVec3,
}

impl Grid {
    /// The grid over `(min, max)`: cubes of [`CELL_SIZE`], or larger until at most
    /// [`MAX_CELLS`] cover the bounds and their margin.
    pub fn over(min: Vec3, max: Vec3) -> Self {
        let mut cell = CELL_SIZE;
        loop {
            let grid = Self::with_cell(min, max, cell);
            if grid.cell_count() <= MAX_CELLS {
                return grid;
            }
            cell *= 1.25;
        }
    }

    fn with_cell(min: Vec3, max: Vec3, cell: f32) -> Self {
        let span = (max - min).max(Vec3::ZERO);
        let inner = (span / cell).ceil().as_uvec3().max(UVec3::ONE);
        Self {
            origin: min - Vec3::splat(cell),
            cell,
            dims: inner + UVec3::splat(2),
        }
    }

    pub fn cell_count(&self) -> usize {
        self.dims.x as usize * self.dims.y as usize * self.dims.z as usize
    }

    /// The cell index holding `point`, `None` outside the grid (or for a NaN).
    pub fn index_of(&self, point: Vec3) -> Option<usize> {
        let at = (point - self.origin) / self.cell;
        let inside = at.cmpge(Vec3::ZERO).all() && at.cmplt(self.dims.as_vec3()).all();
        inside.then(|| self.index(at.as_uvec3().min(self.dims - UVec3::ONE)))
    }

    /// The index of cell `at`: x fastest, then z, then y.
    pub fn index(&self, at: UVec3) -> usize {
        let d = self.dims;
        ((at.y * d.z + at.z) * d.x + at.x) as usize
    }

    /// Cell `index`'s coordinates.
    pub fn coords(&self, index: usize) -> UVec3 {
        let d = self.dims;
        let i = index as u32;
        UVec3::new(i % d.x, i / (d.x * d.z), (i / d.x) % d.z)
    }

    /// Cell `index`'s box.
    pub fn bounds(&self, index: usize) -> (Vec3, Vec3) {
        let min = self.origin + self.coords(index).as_vec3() * self.cell;
        (min, min + Vec3::splat(self.cell))
    }

    /// Cell `index` and its up to 26 neighbours, in index order.
    pub fn neighbourhood(&self, index: usize) -> impl Iterator<Item = usize> + '_ {
        let at = self.coords(index).as_ivec3();
        let dims = self.dims.as_ivec3();
        (-1..=1).flat_map(move |y| {
            (-1..=1).flat_map(move |z| {
                (-1..=1).filter_map(move |x| {
                    let n = at + glam::IVec3::new(x, y, z);
                    let inside = n.cmpge(glam::IVec3::ZERO).all() && n.cmplt(dims).all();
                    inside.then(|| self.index(n.as_uvec3()))
                })
            })
        })
    }
}
