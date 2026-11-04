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

use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Instant;

use crate::dev::bake::lightmap::logged::watch;
use crate::dev::bake::progress::{BakeLog, Progress, Step};
use crate::scene::lighting::occlusion::{
    gather_occlusion, OcclusionBakeReport, OcclusionData, OcclusionInput, OcclusionProgress,
};
use crate::scene::Scene;

/// A finished occlusion bake: its PVS (`None` when nothing occludes), its report,
/// and the wall-clock milliseconds it took.
#[derive(Debug)]
pub struct BakedOcclusion {
    pub data: Option<OcclusionData>,
    pub report: OcclusionBakeReport,
    pub ms: u64,
}

type Baked = Option<(Option<OcclusionData>, OcclusionBakeReport)>;

/// An occlusion bake on a worker thread.
pub struct OcclusionBakeJob {
    progress: Arc<OcclusionProgress>,
    worker: Option<JoinHandle<Baked>>,
    started: Instant,
}

impl OcclusionBakeJob {
    /// Gather `scene`'s statics and start baking them on a worker.
    pub fn start(scene: &Scene) -> Result<Self, String> {
        let input = gather_occlusion(scene);
        let progress = Arc::new(OcclusionProgress::default());
        let watch = Arc::clone(&progress);
        let worker = thread::Builder::new()
            .name("occlusion-bake".into())
            .spawn(move || input.bake(threads(), &watch))
            .map_err(|e| format!("start the occlusion bake: {e}"))?;
        Ok(Self {
            progress,
            worker: Some(worker),
            started: Instant::now(),
        })
    }

    /// Cells baked and the grid's (0 until it laid its grid).
    pub fn cells(&self) -> (usize, usize) {
        self.progress.cells()
    }

    /// `None` while the worker runs; once it has finished, its bake or why it has
    /// none. The job is spent after that.
    pub fn poll(&mut self) -> Option<Result<BakedOcclusion, String>> {
        if !self.worker.as_ref()?.is_finished() {
            return None;
        }
        let ms = self.started.elapsed().as_millis() as u64;
        Some(match self.worker.take()?.join() {
            Ok(Some((data, report))) => Ok(BakedOcclusion { data, report, ms }),
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
/// `Lighting.Generate()`). An off log bakes on this thread's workers with no watcher.
pub fn bake_occlusion_logged(scene: &Scene, log: &mut BakeLog) -> Result<BakedOcclusion, String> {
    let started = Instant::now();
    let input = gather_occlusion(scene);
    let progress = OcclusionProgress::default();
    let run = |p: &OcclusionProgress| bake(&input, p);
    let (data, report) = match log.is_on() {
        true => watch(&progress, read, log, run)?,
        false => run(&progress)?,
    };
    let ms = started.elapsed().as_millis() as u64;
    Ok(BakedOcclusion { data, report, ms })
}

fn bake(
    input: &OcclusionInput,
    progress: &OcclusionProgress,
) -> Result<(Option<OcclusionData>, OcclusionBakeReport), String> {
    input
        .bake(threads(), progress)
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

/// The workers a bake uses: every core.
fn threads() -> usize {
    thread::available_parallelism().map_or(1, |n| n.get())
}
