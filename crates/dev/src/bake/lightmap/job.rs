use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use super::encoder::Encoder;
use super::measure::Measure;
use super::preview::Previews;
use super::UNSAVED;
use super::{
    apply_encoded, encode_pages, gather_bake_input, lightmap_receivers, warn_missing_uvs,
    EncodedPages,
};
use crate::bake::progress::{duration, eta, Progress, Step};
use rusty_lighting::lightmap::{
    bake_progressive, BakeProgress, BakeScene, BakeView, MaskChannels, Pass,
};
use rusty_scene::lighting::settings::BakeSettings;
use rusty_scene::Scene;

type Encoded = (
    Result<Option<(EncodedPages, Duration, Duration)>, String>,
    BakeScene,
);

#[derive(Debug, PartialEq)]
pub enum BakeOutcome {
    Baked(usize),
    Cancelled,
    Failed(String),
}

pub struct LightmapBakeJob {
    progress: Arc<BakeProgress>,
    worker: Option<JoinHandle<Encoded>>,
    scene_path: String,
    static_meshes: usize,
    previews: Previews,
    paced_from: Option<(Instant, u64)>,
    measure: Measure,
}

impl LightmapBakeJob {
    pub fn start(
        scene: &mut Scene,
        scene_path: Option<&str>,
        settings: BakeSettings,
        view: Option<BakeView>,
    ) -> Result<Self, String> {
        let scene_path = scene_path.ok_or(UNSAVED)?.to_string();
        super::spill::sweep(&scene_path);
        let started = Instant::now();
        let input = gather_bake_input(scene);
        super::fits::fits(&input, &settings, true)?;
        crate::memory::return_large_frees();
        let (progress, measure) = Measure::start(&settings, started.elapsed());
        let lights = rusty_lighting::lightmap::taken_lights(scene);
        let shadowmask = input.shadowmask.clone();
        let static_meshes = lightmap_receivers(&input);
        let progress = Arc::new(progress);
        let previews = Previews::new(&scene_path);
        let watch = Arc::clone(&progress);
        let view = view.filter(|_| settings.prioritize_view);
        let (page, taken) = (settings.page(), (lights, shadowmask));
        let spill_beside = scene_path.clone();
        let mut encoder = Encoder::start(&previews, page, taken.clone(), Arc::clone(&progress))?;
        let worker = thread::Builder::new()
            .name("lightmap-bake".into())
            .spawn(move || {
                let started = Instant::now();
                let baked = bake_progressive(&input, &settings, &watch, view, &mut encoder);
                drop(encoder);
                let traced = Instant::now();
                let encoded = match baked {
                    Some(maps) => encode_pages(maps, page, &watch, &spill_beside).map(|pages| {
                        let times = (traced - started, traced.elapsed());
                        pages.map(|p| (labelled(p, &taken), times.0, times.1))
                    }),
                    None => Ok(None),
                };
                (encoded, input)
            })
            .map_err(|e| format!("start the bake: {e}"))?;
        Ok(Self {
            progress,
            worker: Some(worker),
            scene_path,
            static_meshes,
            previews,
            paced_from: None,
            measure,
        })
    }

    pub fn show_preview(&mut self, scene: &mut Scene) -> Option<Pass> {
        if self.paced_from.is_none() && self.passes().is_some_and(|p| p.done > 0) {
            self.paced_from = Some((Instant::now(), self.progress.done()));
        }
        let pass = self.previews.show(scene)?;
        log::info!("[LightmapBake] lightmaps {}", pass_line(pass, self.eta()));
        Some(pass)
    }

    pub fn restore(&mut self, scene: &mut Scene) {
        self.previews.restore(scene);
    }

    pub fn passes(&self) -> Option<Pass> {
        self.progress.passes()
    }

    pub fn eta(&self) -> Option<Duration> {
        let (at, then) = self.paced_from?;
        let (done, total) = self.progress();
        let now = Progress {
            step: Step::Lightmaps,
            done,
            total,
            meshes: None,
        };
        eta((Duration::ZERO, then), at.elapsed(), &now)
    }

    pub fn progress(&self) -> (u64, u64) {
        (self.progress.done(), self.progress.total())
    }

    pub fn pages(&self) -> Option<(u64, u64)> {
        self.progress.pages()
    }

    pub fn fraction(&self) -> f32 {
        let (done, total) = self.pages().unwrap_or_else(|| self.progress());
        if total == 0 {
            0.0
        } else {
            (done as f64 / total as f64) as f32
        }
    }

    pub fn cancel(&self) {
        self.progress.cancel();
    }

    pub fn is_cancelled(&self) -> bool {
        self.progress.is_cancelled()
    }

    pub fn scene_path(&self) -> &str {
        &self.scene_path
    }

    pub fn is_finished(&self) -> bool {
        self.worker.as_ref().is_none_or(|w| w.is_finished())
    }

    pub fn poll(&mut self, scene: &mut Scene) -> Option<BakeOutcome> {
        if !self.worker.as_ref()?.is_finished() {
            return None;
        }
        let joined = self.worker.take()?.join();
        Some(match joined {
            Ok((Ok(Some(_)), _)) if self.progress.is_cancelled() => BakeOutcome::Cancelled,
            Ok((Ok(Some((pages, trace, encode))), input)) => {
                warn_missing_uvs(self.static_meshes, pages.maps);
                let live = (pages.entries.iter())
                    .filter(|e| scene.world.contains(e.entity))
                    .count();
                let writing = Instant::now();
                match apply_encoded(scene, &self.scene_path, &pages) {
                    Ok(_) => {
                        let times = [trace, encode, writing.elapsed()];
                        let at = (&*scene, self.scene_path.as_str());
                        self.measure.keep(at, &input, &pages, times);
                        BakeOutcome::Baked(live)
                    }
                    Err(e) => BakeOutcome::Failed(e),
                }
            }
            Ok((Ok(None), _)) => BakeOutcome::Cancelled,
            Ok((Err(e), _)) => BakeOutcome::Failed(e),
            Err(_) => BakeOutcome::Failed("the bake worker panicked".into()),
        })
        .inspect(|outcome| {
            if !matches!(outcome, BakeOutcome::Baked(_)) {
                self.previews.restore(scene);
            }
        })
    }
}

impl Drop for LightmapBakeJob {
    fn drop(&mut self) {
        self.progress.cancel();
        self.previews.clear();
    }
}

pub(super) type Taken = (rusty_lighting::lightmap::TakenLights, MaskChannels);

pub(super) fn labelled(pages: EncodedPages, (lights, shadowmask): &Taken) -> EncodedPages {
    EncodedPages {
        lights: lights.clone(),
        shadowmask: shadowmask.clone(),
        ..pages
    }
}

pub fn pass_line(pass: Pass, eta: Option<Duration>) -> String {
    if pass.done == 0 {
        return "first pass".into();
    }
    let left = eta.map_or(String::new(), |t| format!(", ~{} left", duration(t)));
    let view = match pass.view > pass.samples {
        true => format!(", {} in view", pass.view),
        false => String::new(),
    };
    format!(
        "pass {}/{} ({}/{} samples{view}){left}",
        pass.done, pass.passes, pass.samples, pass.target
    )
}
