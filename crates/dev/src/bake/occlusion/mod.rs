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

pub fn reach(scene: &Scene, nav: Option<&NavigationGraph>) -> Reach {
    let fresh = nav.filter(|n| n.is_current_for(scene));
    let floor = fresh.and_then(|n| n.floor_grid(scene.nav_settings.agent_radius, None));
    floor.map_or_else(Reach::everywhere, |f| Reach::over(scene, &f))
}

type Baked = Option<(Option<OcclusionData>, OcclusionBakeReport)>;

pub struct OcclusionBakeJob {
    progress: Arc<OcclusionProgress>,
    clocks: Arc<CellClocks>,
    worker: Option<JoinHandle<Baked>>,
    reach: Reach,
    started: Instant,
    kept: Option<BakedOcclusion>,
}

impl OcclusionBakeJob {
    pub fn start(scene: &Scene, reach: Reach) -> Result<Self, String> {
        let input = gather_with(scene, &Fingerprints::of_scene(scene), reach.clone());
        Self::bake(input, reach)
    }

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

    pub fn cells(&self) -> (usize, usize) {
        self.progress.cells()
    }

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
    fn drop(&mut self) {
        self.progress.cancel();
    }
}

pub fn bake_occlusion_logged(
    scene: &Scene,
    reach: Reach,
    log: &mut BakeLog,
) -> Result<BakedOcclusion, String> {
    let started = Instant::now();
    let input = gather_with(scene, &Fingerprints::of_scene(scene), reach.clone());
    bake_logged(input, &reach, started, log)
}

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
