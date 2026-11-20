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
//! Both runs lend the bake [`CellClocks`], so each cell's sampling time reaches
//! the bake report's `slowestCells` (#1987) without a clock inside the sim.

use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crate::bake::lightmap::logged::watch;
use crate::bake::progress::{BakeLog, Progress, Step};
use rusty_lighting::occlusion::{
    current_pvs, gather_occlusion, OcclusionBakeReport, OcclusionInput, OcclusionProgress,
};
use rusty_scene::lighting::occlusion::OcclusionData;
use rusty_scene::Scene;

mod clocks;

pub use clocks::CellClocks;

/// A finished occlusion bake: its PVS (`None` when nothing occludes or the level is too large to cull), its report,
/// the wall-clock milliseconds it took, and each cell's sampling time by cell
/// index (#1987; empty when no grid was laid). Or, when `kept`, the PVS the scene
/// already holds, unchanged (#2348): no `data` to apply, the held PVS's report,
/// the milliseconds the comparison took and no cell times.
#[derive(Debug)]
pub struct BakedOcclusion {
    pub data: Option<OcclusionData>,
    pub report: OcclusionBakeReport,
    pub ms: u64,
    pub cells: Vec<Duration>,
    pub kept: bool,
}

impl BakedOcclusion {
    /// The PVS `scene` holds, kept, when it is what a bake would write
    /// ([`current_pvs`]): Generate Lighting's one keep-or-bake decision, the
    /// editor's and the script's. `None`: bake.
    pub fn kept(scene: &Scene) -> Option<Self> {
        let started = Instant::now();
        let held = current_pvs(scene)?;
        Some(Self {
            data: None,
            report: OcclusionBakeReport::of_kept(held),
            ms: started.elapsed().as_millis() as u64,
            cells: Vec::new(),
            kept: true,
        })
    }

    /// Put this bake on `scene`: its PVS, or, when kept, the file's ownership
    /// alone ([`Scene::keep_baked_occlusion`]), so the next save writes the kept
    /// PVS as a bake's would.
    pub fn put_on(self, scene: &mut Scene) {
        match self.kept {
            true => scene.keep_baked_occlusion(),
            false => scene.put_baked_occlusion(self.data),
        }
    }
}

type Baked = Option<(Option<OcclusionData>, OcclusionBakeReport)>;

/// An occlusion bake on a worker thread.
pub struct OcclusionBakeJob {
    progress: Arc<OcclusionProgress>,
    clocks: Arc<CellClocks>,
    worker: Option<JoinHandle<Baked>>,
    started: Instant,
    /// Generate Lighting's held PVS, unchanged: finished before it began.
    kept: Option<BakedOcclusion>,
}

impl OcclusionBakeJob {
    /// Gather `scene`'s statics and start baking them on a worker: Bake Occlusion,
    /// which always bakes.
    pub fn start(scene: &Scene) -> Result<Self, String> {
        Self::bake(gather_occlusion(scene))
    }

    /// Generate Lighting's occlusion step: [`Self::start`], unless the scene holds
    /// the PVS that bake would write ([`BakedOcclusion::kept`]), which the job then
    /// hands back finished, on its first [`Self::poll`].
    pub fn generate(scene: &Scene) -> Result<Self, String> {
        let Some(kept) = BakedOcclusion::kept(scene) else {
            return Self::start(scene);
        };
        Ok(Self {
            progress: Arc::default(),
            clocks: Arc::default(),
            worker: None,
            started: Instant::now(),
            kept: Some(kept),
        })
    }

    fn bake(input: OcclusionInput) -> Result<Self, String> {
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
            started: Instant::now(),
            kept: None,
        })
    }

    /// Cells baked and the grid's (0 until it laid its grid).
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

/// Bake `scene`'s PVS in line, saying its cells to `log` (a script's
/// `Lighting.BakeOcclusion()`). An off log bakes on this thread's workers with no watcher.
pub fn bake_occlusion_logged(scene: &Scene, log: &mut BakeLog) -> Result<BakedOcclusion, String> {
    let started = Instant::now();
    let input = gather_occlusion(scene);
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
    log: &mut BakeLog,
) -> Result<BakedOcclusion, String> {
    let Some(kept) = BakedOcclusion::kept(scene) else {
        return bake_occlusion_logged(scene, log);
    };
    log.kept(Step::Occlusion);
    Ok(kept)
}

fn bake(
    input: &OcclusionInput,
    progress: &OcclusionProgress,
) -> Result<(Option<OcclusionData>, OcclusionBakeReport), String> {
    input
        .bake(crate::bake::report::threads(), progress)
        .ok_or_else(|| "never cancelled".to_string())
}

/// The warning a bake gives when the scene file at `path` lacks, or holds
/// otherwise, a material the baked meshes draw (#1958,
/// [`unsaved_materials`](rusty_lighting::occlusion::unsaved_materials)):
/// the PVS fingerprints the live library, so the next load of the file would read
/// it as stale. `None` when the file agrees, or cannot be read.
pub fn unsaved_materials_warning(scene: &Scene, path: &str) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    let saved = rusty_scene::parse_scene_text(&text, path).ok()?.materials;
    let keys = rusty_lighting::occlusion::unsaved_materials(scene, &saved);
    if keys.is_empty() {
        return None;
    }
    let keys: Vec<_> = keys.iter().map(|k| format!("{k:?}")).collect();
    Some(format!(
        "[Occlusion] material(s) {} are defined in memory but not as the scene file holds \
         them: save the scene so its materials are written, or the bake won't match the \
         next load",
        keys.join(", ")
    ))
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
