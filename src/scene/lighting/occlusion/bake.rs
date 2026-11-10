//! The occlusion bake (#1714): which baked objects each cell may see, sampled by
//! `baking`, then dilated over each cell's neighbours and folded into shared rows. Gathered on the caller's thread
//! ([`gather_occlusion`]) and baked on any ([`OcclusionInput::bake`]), so the editor
//! bakes on a worker and a script in line, the same bytes either way.
//!
//! A pure function of the scene and [`OCCLUSION_SEED`]: each cell draws from its own
//! stream and is baked alone, so the result is the same at any worker count.

use std::collections::BTreeMap;

use super::gather::{Fingerprints, Object};
use super::progress::OcclusionProgress;
use super::sample::Baking;
use super::{BakedObject, Grid, OcclusionData};
use crate::scene::Scene;

/// The bake's seed: one fixed stream, so a re-bake of an unchanged scene writes
/// the same bytes.
pub const OCCLUSION_SEED: u64 = 0x5056_5321;

/// What one occlusion bake found. All zero when it baked nothing.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct OcclusionBakeReport {
    /// Static meshes baked as occludees.
    pub objects: usize,
    /// Of them, the ones that occlude.
    pub occluders: usize,
    /// Of them, the ones too large to sample (`targets::MAX_TARGETS`) or with a
    /// corner that is not finite, kept in sight of every cell.
    pub never_culled: usize,
    pub cells: usize,
    /// The cells' edge, metres (0 when no grid was laid).
    pub cell_size: f32,
    /// Whether the level needs cells over `grid::MAX_CELL`, so nothing was baked
    /// and nothing is occlusion-culled.
    pub too_large: bool,
    /// Distinct visible sets the cells share.
    pub rows: usize,
    /// Bytes of the stored data.
    pub bytes: usize,
}

/// The static scene as the bake reads it, owned, so a worker thread can bake it
/// while the scene goes on ([`gather_occlusion`]).
pub struct OcclusionInput {
    objects: Vec<Object>,
}

/// What `scene`'s occlusion bake reads, gathered on the caller's thread.
pub fn gather_occlusion(scene: &Scene) -> OcclusionInput {
    let prints = Fingerprints::of_scene(scene);
    let mut ids = scene.world.ids_with_mesh();
    ids.sort_unstable();
    let objects = ids.iter().filter_map(|&id| prints.object(scene, id));
    OcclusionInput {
        objects: objects.collect(),
    }
}

/// Bake `scene`'s PVS on `threads` workers, in line. `None` when there is nothing
/// to cull (no static mesh, none that occludes, none small and finite enough to
/// sample) or the level needs cells over `MAX_CELL`.
pub fn bake_occlusion(
    scene: &Scene,
    threads: usize,
) -> (Option<OcclusionData>, OcclusionBakeReport) {
    let input = gather_occlusion(scene);
    input
        .bake(threads, &OcclusionProgress::default())
        .expect("nobody cancels it")
}

impl OcclusionInput {
    /// The scene ready to sample and its grid, for a measurement that times or
    /// counts cells one by one; `None` when no grid is laid.
    #[cfg(all(test, feature = "dev"))]
    pub(crate) fn sampler(&self) -> Option<Sampler> {
        let baking = Baking::of(&self.objects);
        let (min, max) = baking.bounds?;
        let grid = Grid::over(min, max)?;
        Some(Sampler { baking, grid })
    }

    /// The index entity `entity` has in what a cell sees, `None` when the bake
    /// reads no object for it.
    #[cfg(all(test, feature = "dev"))]
    pub(crate) fn object(&self, entity: u32) -> Option<usize> {
        self.objects.iter().position(|o| o.entity == entity)
    }

    /// Bake `count` cells spread over the grid on this thread, for a measurement
    /// of a level too large to bake whole in a test: the grid's cell count and
    /// edge, and the sheets the occluders fused into (count, bytes). `None` when
    /// no grid is laid.
    #[cfg(all(test, feature = "dev"))]
    pub(crate) fn bake_sample(&self, count: usize) -> Option<(usize, f32, (usize, usize))> {
        let sampler = self.sampler()?;
        sampler.spread(count as u64);
        let grid = &sampler.grid;
        Some((grid.cell_count(), grid.cell, sampler.baking.sheets()))
    }

    /// Bake the PVS on `threads` workers, counting into `progress`; `None` when it
    /// was cancelled, else the data (`None` when nothing occludes, nothing can be
    /// culled, or the level is too large: the report says which) and the report.
    pub fn bake(
        &self,
        threads: usize,
        progress: &OcclusionProgress,
    ) -> Option<(Option<OcclusionData>, OcclusionBakeReport)> {
        let objects = &self.objects;
        let occluders = objects.iter().filter(|o| !o.occluding.is_empty()).count();
        if occluders == 0 {
            return Some((None, OcclusionBakeReport::default()));
        }
        let scene = Baking::of(objects);
        let mut report = OcclusionBakeReport {
            objects: objects.len(),
            occluders,
            never_culled: scene.never_culled(),
            ..Default::default()
        };
        // Nothing to cull, or a level too large to cull: no PVS, said in the report.
        let Some((min, max)) = scene.bounds else {
            return Some((None, report));
        };
        let Some(grid) = Grid::over(min, max) else {
            report.too_large = true;
            return Some((None, report));
        };
        progress.start(grid.cell_count());
        let seen = scene.cells(&grid, threads.max(1), progress)?;
        let (rows, cells) = fold_rows(&grid, &seen, objects.len().div_ceil(64));
        let baked = objects.iter().map(|o| BakedObject {
            entity: o.entity,
            fingerprint: o.fingerprint,
            occluder: !o.occluding.is_empty(),
        });
        let data = OcclusionData::new(grid, baked.collect(), rows, cells);
        let data = data.expect("rows fit the grid");
        let report = OcclusionBakeReport {
            cells: grid.cell_count(),
            cell_size: grid.cell,
            rows: data.row_count(),
            bytes: super::encode(&data).len(),
            ..report
        };
        Some((Some(data), report))
    }
}

/// One bake's sampling, a cell at a time ([`OcclusionInput::sampler`]).
#[cfg(all(test, feature = "dev"))]
pub(crate) struct Sampler {
    baking: Baking,
    pub grid: Grid,
}

#[cfg(all(test, feature = "dev"))]
impl Sampler {
    /// What cell `index` sees on its own, as the bake samples it.
    pub fn cell(&self, index: usize) -> Vec<u64> {
        self.baking.cell(&self.grid, index)
    }

    /// Bake `count` cells spread over the grid, one after another, on this
    /// thread: the same cells for the same level and count.
    pub fn spread(&self, count: u64) {
        let cells = self.grid.cell_count() as u64;
        for k in 0..count {
            let cell = k.wrapping_mul(0x9E37_79B9_7F4A_7C15) % cells;
            std::hint::black_box(self.cell(cell as usize));
        }
    }

    /// Rays cast so far, either kind.
    pub fn rays_cast(&self) -> u64 {
        self.baking.rays_cast()
    }
}

/// Each cell's set widened by its neighbours' (the dilation), then the distinct
/// sets as rows in first-seen cell order and each cell's row.
fn fold_rows(grid: &Grid, seen: &[Vec<u64>], words: usize) -> (Vec<u64>, Vec<u32>) {
    let mut ids: BTreeMap<Vec<u64>, u32> = BTreeMap::new();
    let (mut rows, mut cells) = (Vec::new(), Vec::with_capacity(seen.len()));
    for cell in 0..seen.len() {
        let mut bits = vec![0u64; words];
        for n in grid.neighbourhood(cell) {
            bits.iter_mut().zip(&seen[n]).for_each(|(b, s)| *b |= s);
        }
        let next = ids.len() as u32;
        let row = *ids.entry(bits).or_insert_with_key(|bits| {
            rows.extend_from_slice(bits);
            next
        });
        cells.push(row);
    }
    (rows, cells)
}
