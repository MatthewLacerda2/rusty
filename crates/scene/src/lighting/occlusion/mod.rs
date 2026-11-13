//! crates/scene/src/lighting/occlusion/ — baked occlusion culling (#1714, part 2): a
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
//! **Cells.** A uniform 3D grid over the bounds of the static meshes it can cull
//! ([`Grid`]), up to [`MAX_CELL`] a side (past it nothing is culled), not the room
//! finder's rooms: rooms are found on one floor's plan (`placement::rooms`), while a
//! Dust2-sized level has storeys, stairs, drops and open yards a camera moves
//! through. A uniform grid holds all of them. Cells that see the same set share one
//! row (`rows`); after the dilation few do, so the data is about one bit per cell
//! per object (11.8 MB for a Dust2-sized level of 6344 meshes, `docs/api/Lighting.md`).
//!
//! **What a cell holds** (`bake`, `sample`): every baked object any point in it may
//! see. From seeded points in each 2 m cube of the cell (the same density at every
//! cell size), rays go to points on each object's own triangles: its extremes and a
//! lattice at most `TARGET_SPACING` apart, one per `VOXEL` of space, so the count
//! follows its area, not its triangles (`targets`); one ray that no occluder stops
//! before it arrives makes the object visible. Most rays never need casting
//! (part 3): an occluder face, alone or fused with the other faces on its plane (a
//! floor of tiles, a wall of pieces), that hides a cluster of objects, an object, or
//! a chunk of its points from a cube or from one of its points proves their rays
//! stopped, and each cube tries first the faces that last stopped one (`sight`).
//! Every such proof is exact, so they change the bake's speed, not what it sees.
//! An object too large for `MAX_TARGETS` points, or not finite, is never culled and
//! lays no cell. Then every cell takes its 26 neighbours' sets too (the dilation),
//! so a camera anywhere in a cell is covered by samples from the cells around it. The
//! invariant is one way: a cell may keep what is hidden, and must never drop what is
//! visible. The sampling is a pure function of the scene and the seed, and each cell
//! is baked alone, so the bytes are the same at any worker count.
//!
//! **What is baked against** (`gather`): each object's [`fingerprint`], a hash of
//! what the bake reads of it and nothing else (#1958): its triangles corner by
//! corner (so a re-export that moved a doorway with the same counts differs, while
//! one that welds the same corners differently does not), its world matrix, layer,
//! cast mode, its `LODGroup`
//! membership, and each drawn slot's material as an occluder reads it (opaque or
//! not, and its Render Face; never its name). At runtime the renderer culls only an object whose fingerprint is still
//! what was baked, and uses the PVS at all only while every baked occluder still
//! matches: an object moved, added or given a material that occludes differently since the bake is drawn, and an
//! occluder moved or removed turns the bake off until it is re-baked. Never stale
//! pixels.
//!
//! **Stored** (`file`) beside the scene as `<scene>.occlusion`, a small binary file
//! the lighting save writes and the scene load reads; absent, the scene culls
//! nothing by occlusion. A runtime level load that moves incoming ids out of a
//! carried entity's way renames the baked objects with them ([`OcclusionData::remap`]).
//!
//! stored-format: the `<scene>.occlusion` file, the baked PVS and its fingerprint

mod bake;
mod file;
mod gather;
mod grid;
mod progress;
mod sample;
mod unsaved;

pub use bake::{
    bake_occlusion, gather_occlusion, OcclusionBakeReport, OcclusionInput, OCCLUSION_SEED,
};
pub use file::{
    apply_sidecar, decode, encode, load_sidecar, occlusion_path, read_sidecar, save_sidecar,
    OcclusionFile, SUFFIX as OCCLUSION_SUFFIX,
};
pub use gather::{fingerprint, Fingerprints};
pub use grid::{Grid, CELL_SIZE, MAX_CELL, MAX_CELLS};
pub use progress::OcclusionObserver;
pub use progress::OcclusionProgress;
pub use sample::{MAX_TARGETS, TARGET_SPACING, VOXEL};
pub use unsaved::unsaved_materials;

#[cfg(test)]
mod bake_tests;
// Test hooks: the levels other crates' bake, render and cost tests build.
#[cfg(feature = "dev")]
#[doc(hidden)]
pub mod bench;
#[cfg(test)]
mod carry_tests;
#[cfg(test)]
mod dense_tests;
#[cfg(test)]
mod file_tests;
#[cfg(test)]
mod grid_tests;
#[cfg(all(test, feature = "dev"))]
mod opening_tests;
#[cfg(test)]
mod print_tests;
#[cfg(test)]
mod sampling_tests;
#[cfg(all(test, feature = "dev"))]
mod tessellated_tests;
#[doc(hidden)]
pub mod test_gap;
#[doc(hidden)]
pub mod test_level;
#[cfg(test)]
mod unread_tests;
#[cfg(test)]
mod unsaved_tests;
#[cfg(test)]
mod window_tests;

/// A scene's baked PVS (`Scene::occlusion`): `None` until Generate Lighting bakes
/// one, and then nothing is occlusion-culled. Every write moves its revision,
/// which the renderer reads to re-bind it.
pub type SceneOcclusion = crate::revision::Revisioned<Option<std::sync::Arc<OcclusionData>>>;

/// The nearest a camera's near plane may be and still cull by the PVS, metres:
/// half a cell. A wall closer than its near plane is clipped away, so the camera
/// sees what the wall hides from the points round it, which the PVS culls.
pub const MAX_CULLING_NEAR: f32 = CELL_SIZE / 2.0;

/// Whether a camera with `projection` and near plane `near` may cull by the PVS,
/// which says only what is seen from points near its eye: a perspective camera
/// whose near plane clips nothing more than [`MAX_CULLING_NEAR`] away. An
/// orthographic camera (a minimap over a roofless level) sees along parallel rays
/// from a whole plane, not from its eye, so it culls nothing by occlusion.
pub fn culls_from(projection: rusty_components::Projection, near: f32) -> bool {
    let perspective = matches!(projection, rusty_components::Projection::Perspective);
    perspective && near <= MAX_CULLING_NEAR
}

/// One baked object: the entity, what it was baked as ([`fingerprint`]), and
/// whether it occludes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BakedObject {
    pub entity: u32,
    pub fingerprint: u64,
    pub occluder: bool,
}

/// A scene's baked PVS: the cells, the objects, and per cell one row of bits, bit
/// `i` set when object `i` may be seen from the cell.
#[derive(Clone, Debug, PartialEq)]
pub struct OcclusionData {
    pub grid: Grid,
    /// A row's bit `i` is `objects[i]`; baked in entity order.
    pub objects: Vec<BakedObject>,
    /// `(entity, index into objects)`, by entity.
    index: Vec<(u32, u32)>,
    /// `u64` words per row.
    words: usize,
    /// The distinct rows, `words` words each.
    rows: Vec<u64>,
    /// Per cell (`Grid::index` order), its row.
    cells: Vec<u32>,
}

impl OcclusionData {
    /// Assemble the data, `None` when the parts disagree (a row index past the
    /// rows, a cell count other than the grid's, rows not whole, an entity twice).
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
            objects,
            index,
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
        let at = self.index.binary_search_by_key(&entity, |e| e.0).ok()?;
        Some(self.index[at].1)
    }

    /// The same data with each baked entity renamed through `moved` (old id → new),
    /// as a runtime level load moves incoming ids out of a carried entity's way
    /// (`scene::runtime::load`). `None` when two objects would share an id.
    pub fn remap(&self, moved: &std::collections::BTreeMap<u32, u32>) -> Option<Self> {
        let mut objects = self.objects.clone();
        for o in &mut objects {
            o.entity = moved.get(&o.entity).copied().unwrap_or(o.entity);
        }
        Self::new(self.grid, objects, self.rows.clone(), self.cells.clone())
    }

    /// How many distinct rows the cells share.
    pub fn row_count(&self) -> usize {
        self.rows.len().checked_div(self.words).unwrap_or(0)
    }

    /// The bytes the data holds in memory, near enough (the file is a little less:
    /// no entity index).
    pub fn byte_size(&self) -> usize {
        self.objects.len() * 21 + self.rows.len() * 8 + self.cells.len() * 4 + 64
    }

    /// The distinct rows (`words` words each, bit `i` object `i`) and each cell's
    /// row: what the file stores, and what a bake report counts (#1980).
    pub fn parts(&self) -> (&[u64], &[u32]) {
        (&self.rows, &self.cells)
    }
}
