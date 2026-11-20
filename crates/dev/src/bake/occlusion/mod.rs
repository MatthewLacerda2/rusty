//! The occlusion bake's runs (#1714): the editor's on a worker thread, a script's
//! in line with its log. Both gather the static scene on the caller's thread
//! (`occlusion::gather_occlusion`) and bake it with the same
//! `OcclusionInput::bake`, so the bytes never depend on which ran it.
//!
//! Generate Lighting runs it last, after the probes commit: the editor keeps
//! answering while it bakes (#808, #1602), with its cells on the bar. A PVS baked
//! from a scene edited meanwhile is still safe to apply: the renderer culls only
//! objects whose fingerprint is what was baked, and none at all once a baked
//! occluder changed (`render::scene::occlusion`).
//!
//! Generate Lighting keeps the PVS the scene holds when it is what the bake would
//! write (#2348, `rusty_lighting::occlusion::is_current`, read from the
//! fingerprints alone): [`OcclusionBakeJob::generate`] and
//! [`generate_occlusion_logged`] then gather and bake nothing, both through
//! [`BakedOcclusion::kept`]. Bake Occlusion always bakes.
//!
//! Both sample only the cells a camera can be in (#2328, [`reach`]): over the
//! live navmesh's walkable spans when it is baked and current for the scene, else
//! every cell, as the console summary then says.
//!
//! Both runs lend the bake [`CellClocks`], so each cell's sampling time reaches
//! the bake report's `slowestCells` (#1987) without a clock inside the sim.

use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Instant;

use crate::bake::lightmap::logged::watch;
use crate::bake::progress::{BakeLog, Progress, Step};
use rusty_lighting::occlusion::{
    gather_with, Fingerprints, OcclusionBakeReport, OcclusionInput, OcclusionProgress, Reach,
};
use rusty_navigation::NavigationGraph;
use rusty_scene::lighting::occlusion::{FloorSource, OcclusionData};
use rusty_scene::Scene;

mod baked;
mod clocks;
pub(crate) mod summary;

pub use summary::unsaved_materials_warning;

pub use baked::BakedOcclusion;
pub use clocks::CellClocks;

/// Where a camera can be in `scene` (#2328), which decides the cells its PVS
/// bake samples: over `nav`'s every walkable span when it is baked and current
/// for the scene (`NavigationGraph::is_current_for`: the editor's per-frame sync
/// keeps it so), else everywhere. A stale navmesh is never read: its floor may be
/// gone, and a floor that misses where a camera can be would cull less there.
pub fn reach(scene: &Scene, nav: Option<&NavigationGraph>) -> Reach {
    let fresh = nav.filter(|n| n.is_current_for(scene));
    let floor = fresh.and_then(|n| n.floor_grid(scene.nav_settings.agent_radius, None));
    floor.map_or_else(Reach::everywhere, |f| Reach::over(scene, &f))
}

type Baked = Option<(Option<OcclusionData>, OcclusionBakeReport)>;

/// An occlusion bake on a worker thread.
pub struct OcclusionBakeJob {
    progress: Arc<OcclusionProgress>,
    clocks: Arc<CellClocks>,
    worker: Option<JoinHandle<Baked>>,
    /// The cells it samples, for the report's standable cells.
    reach: Reach,
    started: Instant,
    /// Generate Lighting's held PVS, unchanged: finished before it began.
    kept: Option<BakedOcclusion>,
}

impl OcclusionBakeJob {
    /// Gather `scene`'s statics and start baking them on a worker, sampling the
    /// cells `reach` can be in: Bake Occlusion, which always bakes.
    pub fn start(scene: &Scene, reach: Reach) -> Result<Self, String> {
        let input = gather_with(scene, &Fingerprints::of_scene(scene), reach.clone());
        Self::bake(input, reach)
    }

    /// Generate Lighting's occlusion step: [`Self::start`] through the fingerprints
    /// the comparison read, unless the scene holds the PVS that bake would write
    /// ([`BakedOcclusion::kept`]), which the job then hands back finished, on its
    /// first [`Self::poll`].
    pub fn generate(scene: &Scene, reach: Reach) -> Result<Self, String> {
        let kept = match BakedOcclusion::kept(scene, &reach) {
            Ok(kept) => kept,
            Err(prints) => return Self::bake(gather_with(scene, &prints, reach.clone()), reach),
        };
        Ok(Self {
            progress: Arc::default(),
            clocks: Arc::default(),
            worker: None,
            reach,
            started: Instant::now(),
            kept: Some(kept),
        })
    }

    fn bake(input: OcclusionInput, reach: Reach) -> Result<Self, String> {
        let clocks = Arc::new(CellClocks::default());
        let progress = Arc::new(OcclusionProgress::observed(clocks.clone()));
        let watch = Arc::clone(&progress);
        let worker = thread::Builder::new()
            .name("occlusion-bake".into())
            .spawn(move || input.bake(crate::bake::report::threads(), &watch))
            .map_err(|e| format!("start the occlusion bake: {e}"))?;
        Ok(Self {
            progress,
            clocks,
            worker: Some(worker),
            reach,
            started: Instant::now(),
            kept: None,
        })
    }

    /// Cells baked and the cells the bake samples (0 until it laid its grid).
    pub fn cells(&self) -> (usize, usize) {
        self.progress.cells()
    }

    /// `None` while the worker runs; once it has finished, its bake or why it has
    /// none. The job is spent after that.
    pub fn poll(&mut self) -> Option<Result<BakedOcclusion, String>> {
        if let Some(kept) = self.kept.take() {
            return Some(Ok(kept));
        }
        if !self.worker.as_ref()?.is_finished() {
            return None;
        }
        let ms = self.started.elapsed().as_millis() as u64;
        Some(match self.worker.take()?.join() {
            Ok(Some((data, report))) => Ok(BakedOcclusion {
                standable: standable(&self.reach, data.as_ref()),
                data,
                report,
                ms,
                cells: self.clocks.times(),
                kept: false,
            }),
            Ok(None) => Err("the occlusion bake was stopped".into()),
            Err(_) => Err("the occlusion bake worker panicked".into()),
        })
    }
}

impl Drop for OcclusionBakeJob {
    /// A job dropped mid-bake (the scene replaced, the editor closing) stops its
    /// worker rather than leaving it to burn the cores.
    fn drop(&mut self) {
        self.progress.cancel();
    }
}

/// Bake `scene`'s PVS in line, sampling the cells `reach` can be in, saying its
/// cells to `log` (a script's `Lighting.BakeOcclusion()`). An off log bakes on
/// this thread's workers with no watcher.
pub fn bake_occlusion_logged(
    scene: &Scene,
    reach: Reach,
    log: &mut BakeLog,
) -> Result<BakedOcclusion, String> {
    let started = Instant::now();
    let input = gather_with(scene, &Fingerprints::of_scene(scene), reach.clone());
    bake_logged(input, &reach, started, log)
}

/// Bake `input`, gathered from the clock `started` at, saying its cells to `log`.
fn bake_logged(
    input: OcclusionInput,
    reach: &Reach,
    started: Instant,
    log: &mut BakeLog,
) -> Result<BakedOcclusion, String> {
    let clocks = Arc::new(CellClocks::default());
    let progress = OcclusionProgress::observed(clocks.clone());
    let run = |p: &OcclusionProgress| bake(&input, p);
    let (data, report) = match log.is_on() {
        true => watch(&progress, read, log, run)?,
        false => run(&progress)?,
    };
    let ms = started.elapsed().as_millis() as u64;
    let cells = clocks.times();
    Ok(BakedOcclusion {
        standable: standable(reach, data.as_ref()),
        data,
        report,
        ms,
        cells,
        kept: false,
    })
}

/// A script's `Lighting.Generate()`'s occlusion step: [`bake_occlusion_logged`],
/// unless the scene holds the PVS it would write ([`BakedOcclusion::kept`]), which
/// is kept and said to `log`.
pub fn generate_occlusion_logged(
    scene: &Scene,
    reach: Reach,
    log: &mut BakeLog,
) -> Result<BakedOcclusion, String> {
    let started = Instant::now();
    match BakedOcclusion::kept(scene, &reach) {
        Ok(kept) => {
            log.kept(Step::Occlusion);
            Ok(kept)
        }
        Err(prints) => {
            let input = gather_with(scene, &prints, reach.clone());
            bake_logged(input, &reach, started, log)
        }
    }
}

/// The cells of `data`'s grid a camera can be in over `reach`, `None` when every
/// cell was sampled.
fn standable(reach: &Reach, data: Option<&OcclusionData>) -> Option<Vec<bool>> {
    let data = data.filter(|d| d.stamp.floor != FloorSource::Everywhere)?;
    reach.standable(&data.grid)
}

fn bake(
    input: &OcclusionInput,
    progress: &OcclusionProgress,
) -> Result<(Option<OcclusionData>, OcclusionBakeReport), String> {
    input
        .bake(crate::bake::report::threads(), progress)
        .ok_or_else(|| "never cancelled".to_string())
}

/// The bake's counters as the log reads them.
fn read(progress: &OcclusionProgress) -> Progress {
    let (done, total) = progress.cells();
    Progress {
        step: Step::Occlusion,
        done: done as u64,
        total: total as u64,
        meshes: None,
    }
}

#[cfg(test)]
mod cost_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod town_cost_tests;
