//! stored-format: the `<scene>.occlusion` file and the baked PVS (its fingerprint: `gather`)

mod file;
pub mod gather;
mod grid;
mod sidecar;

pub use file::{
    decode, encode, encoded_len, occlusion_path, version_of, SUFFIX as OCCLUSION_SUFFIX,
};
pub use grid::{Grid, CELL_SIZE, MAX_CELL, MAX_CELLS, MAX_GRID_CELLS};
pub use sidecar::{apply_sidecar, load_sidecar, read_sidecar, save_sidecar, OcclusionFile};

#[cfg(test)]
mod grid_tests;
#[cfg(test)]
mod unread_tests;

pub type SceneOcclusion = crate::revision::Revisioned<Option<std::sync::Arc<OcclusionData>>>;

pub const MAX_CULLING_NEAR: f32 = CELL_SIZE / 2.0;

pub fn culls_from(projection: rusty_components::Projection, near: f32) -> bool {
    let perspective = matches!(projection, rusty_components::Projection::Perspective);
    perspective && near <= MAX_CULLING_NEAR
}

/// Bump when the bake would write other bytes for the same objects (sampling, dilation,
/// seed, grid limits and growth, sub-cubes, reach, targets) or the file's layout changes;
/// `version_tests` pins small levels only.
pub const OCCLUSION_VERSION: u32 = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BakedObject {
    pub entity: u32,
    pub fingerprint: u64,
    pub occluder: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BakeStamp {
    pub version: u32,
    pub never_culled: u32,
    pub floor: FloorSource,
    pub baked: u32,
    /// Hash of the seeded cells; 0 when all were sampled.
    pub reach: u64,
    /// What `Grid::fitting` laid the grid over; a kept PVS lays it again.
    pub bounds: (glam::Vec3, glam::Vec3),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FloorSource {
    #[default]
    Everywhere,
    Navmesh,
}

impl FloorSource {
    pub fn name(self) -> &'static str {
        match self {
            Self::Everywhere => "everywhere",
            Self::Navmesh => "navmesh",
        }
    }

    pub fn code(self) -> u32 {
        self as u32
    }

    pub fn of_code(code: u32) -> Option<Self> {
        [Self::Everywhere, Self::Navmesh]
            .get(code as usize)
            .copied()
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct OcclusionData {
    pub grid: Grid,
    pub stamp: BakeStamp,
    /// Bit `i` of a row is `objects[i]`.
    pub objects: Vec<BakedObject>,
    index: Vec<(u32, u32)>,
    /// `u64` words per row.
    words: usize,
    rows: Vec<u64>,
    /// Per cell, in `Grid::index` order, its row.
    cells: Vec<u32>,
}

impl OcclusionData {
    pub fn new(
        grid: Grid,
        objects: Vec<BakedObject>,
        rows: Vec<u64>,
        cells: Vec<u32>,
    ) -> Option<Self> {
        let words = objects.len().div_ceil(64);
        let count = rows.len().checked_div(words).unwrap_or(0);
        let whole = rows.len() == count * words;
        let cells_ok =
            cells.len() == grid.cell_count() && cells.iter().all(|&r| (r as usize) < count.max(1));
        let index = objects.iter().enumerate();
        let mut index: Vec<(u32, u32)> = index.map(|(i, o)| (o.entity, i as u32)).collect();
        index.sort_unstable();
        let unique = index.windows(2).all(|w| w[0].0 < w[1].0);
        (whole && cells_ok && unique && (words == 0 || count > 0)).then_some(Self {
            grid,
            stamp: BakeStamp {
                version: OCCLUSION_VERSION,
                baked: grid.cell_count() as u32,
                ..BakeStamp::default()
            },
            objects,
            index,
            words,
            rows,
            cells,
        })
    }

    pub fn row_at(&self, point: glam::Vec3) -> Option<u32> {
        self.grid.index_of(point).map(|cell| self.cells[cell])
    }

    pub fn sees(&self, row: u32, object: u32) -> bool {
        let word = row as usize * self.words + object as usize / 64;
        self.rows
            .get(word)
            .is_none_or(|w| w & (1u64 << (object % 64)) != 0)
    }

    pub fn object_of(&self, entity: u32) -> Option<u32> {
        let at = self.index.binary_search_by_key(&entity, |e| e.0).ok()?;
        Some(self.index[at].1)
    }

    pub fn remap(&self, moves: &crate::runtime::renumber::Moves) -> Option<Self> {
        let mut objects = self.objects.clone();
        for o in &mut objects {
            o.entity = moves.to(o.entity);
        }
        let data = Self::new(self.grid, objects, self.rows.clone(), self.cells.clone())?;
        Some(data.stamped(self.stamp))
    }

    pub fn stamped(self, stamp: BakeStamp) -> Self {
        Self { stamp, ..self }
    }

    pub fn row_count(&self) -> usize {
        self.rows.len().checked_div(self.words).unwrap_or(0)
    }

    /// Near enough: 13 file bytes and 8 of index an object.
    pub fn byte_size(&self) -> usize {
        self.objects.len() * 21 + self.rows.len() * 8 + self.cells.len() * 4 + 72
    }

    pub fn parts(&self) -> (&[u64], &[u32]) {
        (&self.rows, &self.cells)
    }
}
