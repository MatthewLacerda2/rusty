use glam::{UVec3, Vec3};

pub const CELL_SIZE: f32 = 2.0;

pub const MAX_CELLS: usize = 16_384;

pub const MAX_GRID_CELLS: usize = 4 * MAX_CELLS;

pub const MAX_CELL: f32 = 4.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Grid {
    pub origin: Vec3,
    pub cell: f32,
    pub dims: UVec3,
}

impl Grid {
    pub fn over(min: Vec3, max: Vec3) -> Option<Self> {
        Self::fitting(min, max, |g| (g.cell_count(), ())).map(|(grid, ())| grid)
    }

    pub fn fitting<T>(
        min: Vec3,
        max: Vec3,
        mut sampled: impl FnMut(&Self) -> (usize, T),
    ) -> Option<(Self, T)> {
        if !(min.is_finite() && max.is_finite()) {
            return None;
        }
        let mut cell = CELL_SIZE;
        while cell <= MAX_CELL {
            let grid = Self::with_cell(min, max, cell);
            if grid.cell_count() <= MAX_GRID_CELLS {
                let (count, marked) = sampled(&grid);
                if count <= MAX_CELLS {
                    return Some((grid, marked));
                }
            }
            cell *= 1.25;
        }
        None
    }

    fn with_cell(min: Vec3, max: Vec3, cell: f32) -> Self {
        let span = (max - min).max(Vec3::ZERO);
        let inner = (span / cell).ceil().min(Vec3::splat(MAX_CELLS as f32));
        let inner = inner.as_uvec3().max(UVec3::ONE);
        Self {
            origin: min - Vec3::splat(cell),
            cell,
            dims: inner + UVec3::splat(2),
        }
    }

    pub fn cell_count(&self) -> usize {
        let d = self.dims.as_u64vec3();
        d.x.saturating_mul(d.y).saturating_mul(d.z) as usize
    }

    pub fn subdivisions(&self) -> u32 {
        ((self.cell / CELL_SIZE) - 1e-3).ceil().max(1.0) as u32
    }

    pub fn index_of(&self, point: Vec3) -> Option<usize> {
        let at = (point - self.origin) / self.cell;
        let inside = at.cmpge(Vec3::ZERO).all() && at.cmplt(self.dims.as_vec3()).all();
        inside.then(|| self.index(at.as_uvec3().min(self.dims - UVec3::ONE)))
    }

    pub fn index(&self, at: UVec3) -> usize {
        let d = self.dims;
        ((at.y * d.z + at.z) * d.x + at.x) as usize
    }

    pub fn coords(&self, index: usize) -> UVec3 {
        let d = self.dims;
        let i = index as u32;
        UVec3::new(i % d.x, i / (d.x * d.z), (i / d.x) % d.z)
    }

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
