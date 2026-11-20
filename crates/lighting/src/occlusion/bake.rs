//! The occlusion bake (#1714): which baked objects each cell may see, sampled by
//! `baking`, then dilated over each cell's neighbours and folded into shared rows. Gathered on the caller's thread
//! ([`gather_occlusion`]) and baked on any ([`OcclusionInput::bake`]), so the editor
//! bakes on a worker and a script in line, the same bytes either way.
//!
//! A pure function of the scene and [`OCCLUSION_SEED`]: each cell draws from its own
//! stream and is baked alone, so the result is the same at any worker count.

use glam::Vec3;

use super::fold::fold_rows;
use super::gather::{Fingerprints, Object};
use super::progress::OcclusionProgress;
use super::reach::{Cells, Reach};
use super::sample::Baking;
#[cfg(feature = "dev")]
use super::sampler::Sampler;
use rusty_scene::lighting::occlusion::{BakeStamp, BakedObject, FloorSource, OcclusionData};
use rusty_scene::Scene;

/// The bake's seed: one fixed stream, so a re-bake of an unchanged scene writes
/// the same bytes.
pub const OCCLUSION_SEED: u64 = 0x5056_5321;

/// The bake's version ([`rusty_scene::lighting::occlusion::OCCLUSION_VERSION`],
/// where the file's decoder reads it and where its rule is written).
pub use rusty_scene::lighting::occlusion::OCCLUSION_VERSION;

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
    /// Of them, the cells sampled (#2328); every other one sees every object.
    pub baked_cells: usize,
    /// Where the bake found the floor that decided which cells it sampled.
    pub floor: FloorSource,
    /// The share of the static bounds' footprint that floor covers (0 to 1),
    /// `None` with no floor: a small one reads as a stale or partial navmesh.
    pub floor_coverage: Option<f32>,
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
    reach: Reach,
}

/// What `scene`'s occlusion bake reads, gathered on the caller's thread, to
/// sample every cell ([`Reach::everywhere`]).
pub fn gather_occlusion(scene: &Scene) -> OcclusionInput {
    gather_with(scene, &Fingerprints::of_scene(scene), Reach::everywhere())
}

/// What `scene`'s occlusion bake reads, sampling the cells `reach` can be in,
/// through `prints`, whose geometry hashes it reuses: a Generate that compared
/// the held PVS ([`current_pvs`]) and goes on to bake hashes no mesh twice.
pub fn gather_with(scene: &Scene, prints: &Fingerprints, reach: Reach) -> OcclusionInput {
    let objects = walk(scene, |id| prints.object(scene, id));
    OcclusionInput {
        objects: objects.collect(),
        reach,
    }
}

/// What `read` makes of each entity the bake reads, in the bake's order (by
/// entity), skipping those it gives `None` (no part in the bake). The one walk
/// [`gather_with`] bakes and [`is_current`] compares, so the two never disagree on
/// which objects, or in what order.
fn walk<'a, T: 'a>(
    scene: &'a Scene,
    read: impl FnMut(u32) -> Option<T> + 'a,
) -> impl Iterator<Item = T> + 'a {
    let mut ids = scene.world.ids_with_mesh();
    ids.sort_unstable();
    ids.into_iter().filter_map(read)
}

/// Bake `scene`'s PVS on `threads` workers, in line, sampling every cell. `None` when there is nothing
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

/// Whether `held` is what a bake of `scene` sampling the cells `reach` can be in
/// would write (#2348): stamped by this [`OCCLUSION_VERSION`], baked from these
/// very objects, in order, each its entity, fingerprint and occluder flag, and
/// seeded from the same cells (#2328: the grid laid again over the held bounds,
/// and the hash of `reach`'s cells on it, `Cells::key`). The bake reads nothing
/// else: the fingerprint hashes all it reads of an object
/// (`gather::Fingerprints::read`), the grid is laid over the objects' bounds and
/// the cells sampled, and its constants and the seed are the version's. So a
/// match is byte for byte a re-bake's PVS ([`OCCLUSION_SEED`]). Reads the
/// fingerprints and the floor, never a triangle: what Generate Lighting pays to
/// keep an unchanged PVS.
pub fn is_current(scene: &Scene, held: &OcclusionData, reach: &Reach) -> bool {
    matches(scene, held, &Fingerprints::of_scene(scene), reach)
}

/// [`is_current`] through `prints`.
fn matches(scene: &Scene, held: &OcclusionData, prints: &Fingerprints, reach: &Reach) -> bool {
    let (lo, hi) = held.stamp.bounds;
    let seeded = || {
        reach
            .grid(lo, hi)
            .is_some_and(|c| c.grid == held.grid && c.key == held.stamp.reach)
    };
    if held.stamp.version != OCCLUSION_VERSION || !seeded() {
        return false;
    }
    let mut now = walk(scene, |id| {
        let (print, occluder) = prints.of(scene, id)?;
        Some((id, print, occluder))
    });
    let mut was = held
        .objects
        .iter()
        .map(|h| (h.entity, h.fingerprint, h.occluder));
    loop {
        match (now.next(), was.next()) {
            (None, None) => return true,
            (a, b) if a != b => return false,
            _ => {}
        }
    }
}

/// The PVS `scene` holds when it is what a bake sampling `reach` would write
/// ([`is_current`]); otherwise the fingerprints the comparison read, for the bake
/// to gather with ([`gather_with`]).
pub fn current_pvs<'a>(scene: &'a Scene, reach: &Reach) -> Result<&'a OcclusionData, Fingerprints> {
    let prints = Fingerprints::of_scene(scene);
    match scene.occlusion.as_deref() {
        Some(held) if matches(scene, held, &prints, reach) => Ok(held),
        _ => Err(prints),
    }
}

impl OcclusionBakeReport {
    /// The report of the bake that made `data`, read back from the PVS alone: what
    /// Generate Lighting says of a PVS it kept unchanged (#2348).
    pub fn of_kept(data: &OcclusionData) -> Self {
        Self {
            objects: data.objects.len(),
            occluders: data.objects.iter().filter(|o| o.occluder).count(),
            never_culled: data.stamp.never_culled as usize,
            cells: data.grid.cell_count(),
            baked_cells: data.stamp.baked as usize,
            floor: data.stamp.floor,
            floor_coverage: None,
            cell_size: data.grid.cell,
            too_large: false,
            rows: data.row_count(),
            bytes: rusty_scene::lighting::occlusion::encoded_len(data),
        }
    }
}

impl OcclusionInput {
    /// The scene ready to sample and its grid, for a measurement that times or
    /// counts cells one by one, counting its rays; `None` when no grid is laid.
    /// A test hook, as [`Sampler`] is.
    #[cfg(feature = "dev")]
    #[doc(hidden)]
    pub fn sampler(&self) -> Option<Sampler> {
        let baking = Baking::of(&self.objects).counted();
        let (min, max) = baking.bounds?;
        let grid = self.reach.grid(min, max)?.grid;
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
    #[cfg(feature = "dev")]
    #[doc(hidden)]
    pub fn bake_sample(&self, count: usize) -> Option<(usize, f32, (usize, usize))> {
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
        let Some(laid) = self.reach.grid(min, max) else {
            report.too_large = true;
            return Some((None, report));
        };
        if laid.source != self.reach.source {
            log::warn!(
                "[Occlusion] the navmesh marks no cell of the PVS grid where a camera can \
                 be: every cell sampled; re-bake the navmesh over the level"
            );
        }
        let grid = laid.grid;
        let baked = laid.sampled_count();
        progress.start(grid.cell_count(), baked);
        let seen = scene.cells(&grid, threads.max(1), progress, laid.sampled.as_deref())?;
        let data = self.assemble(&laid, &seen, (min, max), report.never_culled);
        let floor_coverage = match laid.source {
            FloorSource::Everywhere => None,
            FloorSource::Navmesh => self.reach.coverage((min, max)),
        };
        let report = OcclusionBakeReport {
            cells: grid.cell_count(),
            baked_cells: baked,
            floor: laid.source,
            floor_coverage,
            cell_size: grid.cell,
            rows: data.row_count(),
            bytes: rusty_scene::lighting::occlusion::encoded_len(&data),
            ..report
        };
        Some((Some(data), report))
    }

    /// The PVS the cells `laid` saw (`seen`), dilated and folded into rows, stamped
    /// with the bake that laid them over `bounds`.
    fn assemble(
        &self,
        laid: &Cells,
        seen: &[Option<Vec<u64>>],
        bounds: (Vec3, Vec3),
        never_culled: usize,
    ) -> OcclusionData {
        let objects = &self.objects;
        let (rows, cells) = fold_rows(&laid.grid, seen, objects.len());
        let held = objects.iter().map(|o| BakedObject {
            entity: o.entity,
            fingerprint: o.fingerprint,
            occluder: !o.occluding.is_empty(),
        });
        let data = OcclusionData::new(laid.grid, held.collect(), rows, cells);
        let stamp = BakeStamp {
            version: OCCLUSION_VERSION,
            never_culled: never_culled as u32,
            floor: laid.source,
            baked: laid.sampled_count() as u32,
            reach: laid.key,
            bounds,
        };
        data.expect("rows fit the grid").stamped(stamp)
    }
}
