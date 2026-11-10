//! A lightmap bake on a worker thread (#808), for the editor's Bake Lightmaps
//! button: the window stays live while the CPU path tracer runs.
//!
//! [`LightmapBakeJob::start`] gathers the owned `BakeScene` on the caller's thread
//! (it borrows the ECS) and hands it to a worker, which bakes and then encodes the
//! pages (`encode_pages`: BC6H and every mip, and the compressed PNG, #1694), so
//! neither blocks a frame; [`LightmapBakeJob::poll`], called each frame, writes the
//! finished bytes and points the scene at them on the caller's thread, through
//! the same [`apply_encoded`] the synchronous `Lighting.BakeLightmaps()` uses. A
//! cancelled bake applies nothing, so the scene keeps its previous lightmaps: a
//! cancel lands during the trace, between two levels of the encode, or after it,
//! up to the `poll` that would have applied it. A bake applied writes its report
//! (`report::lightmaps`, #1980), with each mesh's worker time.

use std::sync::Arc;
use std::thread::{self, JoinHandle};

use std::time::{Duration, Instant};

use super::logged::timed;
use super::UNSAVED;
use super::{
    apply_encoded, gather_bake_input, keep_report, lend_denoiser, lightmap_receivers,
    warn_missing_uvs, EncodedPages,
};
use crate::dev::bake::denoise::Oidn;
use crate::dev::bake::progress::MeshClocks;
use crate::dev::bake::report::{lightmaps::Measured, Stages};
use crate::scene::lighting::lightmap::{BakeProgress, BakeScene, BakeSettings};
use crate::scene::Scene;

/// What the worker hands back: the encoded pages with the trace's and the
/// encode's time, `None` when cancelled, or why encoding failed; and the input it
/// baked, for the report (#1980).
type Encoded = (
    Result<Option<(EncodedPages, Duration, Duration)>, String>,
    BakeScene,
);

/// How a finished bake ended, once [`LightmapBakeJob::poll`] has applied it.
#[derive(Debug, PartialEq)]
pub enum BakeOutcome {
    /// This many lightmaps were written and the scene now points at them.
    Baked(usize),
    /// Stopped by [`LightmapBakeJob::cancel`]; the scene was left untouched.
    Cancelled,
    /// The bake or writing its pages failed.
    Failed(String),
}

/// A bake running on a worker thread.
pub struct LightmapBakeJob {
    progress: Arc<BakeProgress>,
    worker: Option<JoinHandle<Encoded>>,
    scene_path: String,
    /// Static meshes asking for a lightmap, for the missing-lightmap-UV report (#831).
    static_meshes: usize,
    /// What the report reads besides the pages: the settings, the gather's time,
    /// each mesh's worker time and OIDN's (#1980).
    settings: BakeSettings,
    gathered: Duration,
    clocks: Arc<MeshClocks>,
    oidn: Option<Arc<Oidn>>,
}

impl LightmapBakeJob {
    /// Gather `scene` and start baking it on a worker. Errors, without starting,
    /// when the scene was never saved (there is nowhere to write the pages).
    pub fn start(
        scene: &mut Scene,
        scene_path: Option<&str>,
        settings: BakeSettings,
    ) -> Result<Self, String> {
        let scene_path = scene_path.ok_or(UNSAVED)?.to_string();
        let started = Instant::now();
        let input = gather_bake_input(scene);
        let gathered = started.elapsed();
        let lights = crate::scene::lighting::lightmap::taken_lights(scene);
        let shadowmask = input.shadowmask.clone();
        let static_meshes = lightmap_receivers(&input);
        let clocks = Arc::new(MeshClocks::default());
        let (progress, oidn) = lend_denoiser(BakeProgress::observed(clocks.clone()), &settings);
        let progress = Arc::new(progress);
        let watch = Arc::clone(&progress);
        let work = move || {
            let pages = timed(&input, &settings, &watch);
            let with = |(p, trace, encode)| {
                let (lights, shadowmask) = (lights, shadowmask);
                let pages = EncodedPages {
                    lights,
                    shadowmask,
                    ..p
                };
                (pages, trace, encode)
            };
            (pages.map(|p| p.map(with)), input)
        };
        let worker = (thread::Builder::new()
            .name("lightmap-bake".into())
            .spawn(work))
        .map_err(|e| format!("start the bake: {e}"))?;
        Ok(Self {
            progress,
            worker: Some(worker),
            scene_path,
            static_meshes,
            settings,
            gathered,
            clocks,
            oidn,
        })
    }

    /// Texels traced so far and the bake's total (0 until it has rasterized).
    pub fn progress(&self) -> (u64, u64) {
        (self.progress.done(), self.progress.total())
    }

    /// Pages encoded and the pages the encode makes, once the trace is done and
    /// the encode has begun (#1694); `None` before.
    pub fn pages(&self) -> Option<(u64, u64)> {
        self.progress.pages()
    }

    /// Fraction done, 0..=1, for a progress bar: of the texels while tracing, of
    /// the pages while encoding.
    pub fn fraction(&self) -> f32 {
        let (done, total) = self.pages().unwrap_or_else(|| self.progress());
        if total == 0 {
            0.0
        } else {
            (done as f64 / total as f64) as f32
        }
    }

    /// Ask the worker to stop; the next [`poll`](Self::poll) after it does reports
    /// [`BakeOutcome::Cancelled`].
    pub fn cancel(&self) {
        self.progress.cancel();
    }

    pub fn is_cancelled(&self) -> bool {
        self.progress.is_cancelled()
    }

    /// Where the pages will be written beside.
    pub fn scene_path(&self) -> &str {
        &self.scene_path
    }

    /// Whether the worker has finished, so [`Self::poll`] would land its result.
    pub fn is_finished(&self) -> bool {
        self.worker.as_ref().is_none_or(|w| w.is_finished())
    }

    /// `None` while the worker runs. Once it has finished, apply its result to
    /// `scene` and say how it ended; the job is spent after that.
    pub fn poll(&mut self, scene: &mut Scene) -> Option<BakeOutcome> {
        if !self.worker.as_ref()?.is_finished() {
            return None;
        }
        let joined = self.worker.take()?.join();
        Some(match joined {
            // A cancel after the encode finished still applies nothing.
            Ok((Ok(Some(_)), _)) if self.progress.is_cancelled() => BakeOutcome::Cancelled,
            Ok((Ok(Some(baked)), input)) => self.apply(scene, &input, baked),
            Ok((Ok(None), _)) => BakeOutcome::Cancelled,
            Ok((Err(e), _)) => BakeOutcome::Failed(e),
            Err(_) => BakeOutcome::Failed("the bake worker panicked".into()),
        })
    }

    /// Write the finished pages and point `scene` at them, then the report.
    fn apply(
        &self,
        scene: &mut Scene,
        input: &BakeScene,
        (pages, trace, encode): (EncodedPages, Duration, Duration),
    ) -> BakeOutcome {
        warn_missing_uvs(self.static_meshes, pages.maps);
        let writing = Instant::now();
        let n = match apply_encoded(scene, &self.scene_path, &pages) {
            Ok(n) => n,
            Err(e) => return BakeOutcome::Failed(e),
        };
        let mut stages = Stages::start();
        stages.push("gather", self.gathered);
        stages.push("trace", trace);
        stages.push("encode", encode);
        stages.push("write", writing.elapsed());
        let measured = Measured {
            times: self.clocks.times(),
            denoised: self.oidn.as_ref().map(|o| o.spent()),
        };
        let read = (input, &self.settings);
        keep_report(scene, &self.scene_path, read, (&pages, &measured), &stages);
        BakeOutcome::Baked(n)
    }
}

impl Drop for LightmapBakeJob {
    /// A job dropped mid-bake (the editor closing) stops its worker rather than
    /// leaving it to burn the cores.
    fn drop(&mut self) {
        self.progress.cancel();
    }
}
