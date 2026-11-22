use glam::Vec3;

use super::fold::fold_rows;
use super::gather::{Fingerprints, Object};
use super::progress::OcclusionProgress;
use super::reach::{Cells, Reach};
use super::report::OcclusionBakeReport;
use super::sample::Baking;
#[cfg(feature = "dev")]
use super::sampler::Sampler;
use rusty_scene::lighting::occlusion::{BakeStamp, BakedObject, FloorSource, OcclusionData};
use rusty_scene::Scene;

pub const OCCLUSION_SEED: u64 = 0x5056_5321;

pub use rusty_scene::lighting::occlusion::OCCLUSION_VERSION;

pub struct OcclusionInput {
    objects: Vec<Object>,
    reach: Reach,
}

pub fn gather_occlusion(scene: &Scene) -> OcclusionInput {
    gather_with(scene, &Fingerprints::of_scene(scene), Reach::everywhere())
}

pub fn gather_with(scene: &Scene, prints: &Fingerprints, reach: Reach) -> OcclusionInput {
    let objects = walk(scene, |id| prints.object(scene, id));
    OcclusionInput {
        objects: objects.collect(),
        reach,
    }
}

fn walk<'a, T: 'a>(
    scene: &'a Scene,
    read: impl FnMut(u32) -> Option<T> + 'a,
) -> impl Iterator<Item = T> + 'a {
    let mut ids = scene.world.ids_with_mesh();
    ids.sort_unstable();
    ids.into_iter().filter_map(read)
}

pub fn bake_occlusion(
    scene: &Scene,
    threads: usize,
) -> (Option<OcclusionData>, OcclusionBakeReport) {
    let input = gather_occlusion(scene);
    input
        .bake(threads, &OcclusionProgress::default())
        .expect("nobody cancels it")
}

/// Sound only while the fingerprint hashes all the bake reads of an object: a new input joins it.
pub fn is_current(scene: &Scene, held: &OcclusionData, reach: &Reach) -> bool {
    matches(scene, held, &Fingerprints::of_scene(scene), reach)
}

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

pub fn current_pvs<'a>(scene: &'a Scene, reach: &Reach) -> Result<&'a OcclusionData, Fingerprints> {
    let prints = Fingerprints::of_scene(scene);
    match scene.occlusion.as_deref() {
        Some(held) if matches(scene, held, &prints, reach) => Ok(held),
        _ => Err(prints),
    }
}

impl OcclusionInput {
    #[cfg(feature = "dev")]
    /// A test hook: the sampler `dev::bake`'s cost tests drive.
    #[doc(hidden)]
    pub fn sampler(&self) -> Option<Sampler> {
        let baking = Baking::of(&self.objects).counted();
        let (min, max) = baking.bounds?;
        let grid = self.reach.grid(min, max)?.grid;
        Some(Sampler { baking, grid })
    }

    #[cfg(all(test, feature = "dev"))]
    pub(crate) fn object(&self, entity: u32) -> Option<usize> {
        self.objects.iter().position(|o| o.entity == entity)
    }

    #[cfg(feature = "dev")]
    /// A test hook: `dev::bake`'s cost tests sample `count` cells with it.
    #[doc(hidden)]
    pub fn bake_sample(&self, count: usize) -> Option<(usize, f32, (usize, usize))> {
        let sampler = self.sampler()?;
        sampler.spread(count as u64);
        let grid = &sampler.grid;
        Some((grid.cell_count(), grid.cell, sampler.baking.sheets()))
    }

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
        let Some((min, max)) = scene.bounds else {
            return Some((None, report));
        };
        let Some(laid) = self.reach.grid(min, max) else {
            report.too_large = true;
            return Some((None, report));
        };
        let navmesh_missed = laid.source != self.reach.source;
        if navmesh_missed {
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
        let (floor_coverage, play_height) = match laid.source {
            FloorSource::Everywhere => (None, 0.0),
            FloorSource::Navmesh => (self.reach.coverage((min, max)), self.reach.play_height),
        };
        let report = OcclusionBakeReport {
            cells: grid.cell_count(),
            baked_cells: baked,
            floor: laid.source,
            navmesh_missed,
            play_height,
            floor_coverage,
            cell_size: grid.cell,
            rows: data.row_count(),
            bytes: rusty_scene::lighting::occlusion::encoded_len(&data),
            ..report
        };
        Some((Some(data), report))
    }

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
