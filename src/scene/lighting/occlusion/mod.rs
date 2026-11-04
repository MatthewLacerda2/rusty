//! src/scene/lighting/occlusion/ — baked occlusion culling (#1714, part 2): a
//! potentially visible set (PVS) over a grid of cells, baked from the static scene.
//!
//! Unity's Occlusion Culling (Umbra) bakes the same thing from the same inputs:
//! static occluders and static occludees (`StaticEditorFlags`), read at runtime by a
//! camera with `Camera.useOcclusionCulling`. rusty has one static flag, so a static
//! mesh is both: an opaque one occludes and is occluded, a transparent, cutout or
//! custom-shader one is only occluded (its holes or its moving vertices let a view
//! through). A mesh in a `LODGroup`, or with a small-object cull, is only occluded
//! too: what draws of it changes with the camera.
//!
//! **Cells.** A uniform 3D grid over the static bounds ([`Grid`]), not the room
//! finder's rooms: rooms are found on one floor's plan (`placement::rooms`), while a
//! Dust2-sized level has storeys, stairs, drops and open yards a camera moves
//! through. A uniform grid holds all of them. Cells that see the same set share one
//! row (`rows`); after the dilation few do, so the data is about one bit per cell
//! per object (7.8 MB for a Dust2-sized level of 4163 meshes, `docs/api/Lighting.md`).
//!
//! **What a cell holds** (`bake`): every baked object any point in it may see. From
//! seeded points in the cell, rays go to seeded points on each object's own
//! triangles; one ray that no occluder stops before it arrives makes the object
//! visible. Then every cell takes its 26 neighbours' sets too (the dilation), so a
//! camera anywhere in a cell is covered by samples from the cells around it. The
//! invariant is one way: a cell may keep what is hidden, and must never drop what is
//! visible. The sampling is a pure function of the scene and the seed, and each cell
//! is baked alone, so the bytes are the same at any worker count.
//!
//! **What is baked against** (`gather`): each object's [`fingerprint`] (its mesh,
//! world matrix, layer, cast mode, and its slots' materials as an occluder reads
//! them). At runtime the renderer culls only an object whose fingerprint is still
//! what was baked, and uses the PVS at all only while every baked occluder still
//! matches: an object moved, added or re-materialled since the bake is drawn, and an
//! occluder moved or removed turns the bake off until it is re-baked. Never stale
//! pixels.
//!
//! **Stored** (`file`) beside the scene as `<scene>.occlusion`, a small binary file
//! the lighting save writes and the scene load reads; absent, the scene culls
//! nothing by occlusion.

mod bake;
mod file;
mod gather;
mod grid;

pub use bake::{bake_occlusion, OcclusionBakeReport, OCCLUSION_SEED};
pub use file::{decode, encode, load_sidecar, occlusion_path, save_sidecar};
pub use gather::{fingerprint, Fingerprints};
pub use grid::{Grid, CELL_SIZE, MAX_CELLS};

#[cfg(test)]
mod bake_tests;
#[cfg(test)]
mod file_tests;
#[cfg(test)]
mod grid_tests;
#[cfg(test)]
pub(crate) mod test_level;

/// A scene's baked PVS (`Scene::occlusion`): `None` until Generate Lighting bakes
/// one, and then nothing is occlusion-culled. Every write moves its revision,
/// which the renderer reads to re-bind it.
pub type SceneOcclusion = crate::scene::revision::Revisioned<Option<std::sync::Arc<OcclusionData>>>;

/// One baked object: the entity, what it was baked as ([`fingerprint`]), and
/// whether it occludes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BakedObject {
    pub entity: u32,
    pub fingerprint: u64,
    pub occluder: bool,
}

/// A scene's baked PVS: the cells, the objects in entity order, and per cell one
/// row of bits, bit `i` set when object `i` may be seen from the cell.
#[derive(Clone, Debug, PartialEq)]
pub struct OcclusionData {
    pub grid: Grid,
    /// In entity order: a row's bit `i` is `objects[i]`.
    pub objects: Vec<BakedObject>,
    /// `u64` words per row.
    words: usize,
    /// The distinct rows, `words` words each.
    rows: Vec<u64>,
    /// Per cell (`Grid::index` order), its row.
    cells: Vec<u32>,
}

impl OcclusionData {
    /// Assemble the data, `None` when the parts disagree (a row index past the
    /// rows, a cell count other than the grid's, rows not whole).
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
        let sorted = objects.windows(2).all(|w| w[0].entity < w[1].entity);
        (whole && cells_ok && sorted && (words == 0 || count > 0)).then_some(Self {
            grid,
            objects,
            words,
            rows,
            cells,
        })
    }

    /// The row of the cell holding `point`, `None` outside every cell (culls nothing).
    pub fn row_at(&self, point: glam::Vec3) -> Option<u32> {
        self.grid.index_of(point).map(|cell| self.cells[cell])
    }

    /// Whether object `object` (an index into `objects`) may be seen from a cell
    /// whose row is `row`.
    pub fn sees(&self, row: u32, object: u32) -> bool {
        let word = row as usize * self.words + object as usize / 64;
        self.rows
            .get(word)
            .is_none_or(|w| w & (1u64 << (object % 64)) != 0)
    }

    /// Index of `entity` in `objects`, if it was baked.
    pub fn object_of(&self, entity: u32) -> Option<u32> {
        let at = self
            .objects
            .binary_search_by_key(&entity, |o| o.entity)
            .ok()?;
        Some(at as u32)
    }

    /// How many distinct rows the cells share.
    pub fn row_count(&self) -> usize {
        self.rows.len().checked_div(self.words).unwrap_or(0)
    }

    /// The bytes the data holds in memory and on disk, near enough.
    pub fn byte_size(&self) -> usize {
        self.objects.len() * 13 + self.rows.len() * 8 + self.cells.len() * 4 + 64
    }

    pub(super) fn parts(&self) -> (&[u64], &[u32]) {
        (&self.rows, &self.cells)
    }
}
