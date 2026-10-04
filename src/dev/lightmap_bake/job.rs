//! A lightmap bake on a worker thread (#808), for the editor's Bake Lightmaps
//! button: the window stays live while the CPU path tracer runs.
//!
//! [`LightmapBakeJob::start`] gathers the owned `BakeScene` on the caller's thread
//! (it borrows the ECS) and hands it to a worker; [`LightmapBakeJob::poll`], called
//! each frame, applies the finished result back on the caller's thread through the
//! same [`apply_lightmaps`] the synchronous `Lighting.BakeLightmaps()` uses. A
//! cancelled bake applies nothing, so the scene keeps its previous lightmaps.

use std::sync::Arc;
use std::thread::{self, JoinHandle};

use super::{apply_lightmaps, gather_bake_input, UNSAVED};
use crate::scene::lighting::lightmap::{bake_with_progress, BakeProgress, BakeSettings, Lightmap};
use crate::scene::Scene;

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
    worker: Option<JoinHandle<Option<Vec<Lightmap>>>>,
    scene_path: String,
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
        let input = gather_bake_input(scene);
        let progress = Arc::new(BakeProgress::default());
        let watch = Arc::clone(&progress);
        let worker = thread::Builder::new()
            .name("lightmap-bake".into())
            .spawn(move || bake_with_progress(&input, &settings, &watch))
            .map_err(|e| format!("start the bake: {e}"))?;
        Ok(Self {
            progress,
            worker: Some(worker),
            scene_path,
        })
    }

    /// Texels traced so far and the bake's total (0 until it has rasterized).
    pub fn progress(&self) -> (u64, u64) {
        (self.progress.done(), self.progress.total())
    }

    /// Fraction done, 0..=1, for a progress bar.
    pub fn fraction(&self) -> f32 {
        let (done, total) = self.progress();
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

    /// `None` while the worker runs. Once it has finished, apply its result to
    /// `scene` and say how it ended; the job is spent after that.
    pub fn poll(&mut self, scene: &mut Scene) -> Option<BakeOutcome> {
        if !self.worker.as_ref()?.is_finished() {
            return None;
        }
        let joined = self.worker.take()?.join();
        Some(match joined {
            Ok(Some(maps)) => match apply_lightmaps(scene, &self.scene_path, &maps) {
                Ok(n) => BakeOutcome::Baked(n),
                Err(e) => BakeOutcome::Failed(e),
            },
            Ok(None) => BakeOutcome::Cancelled,
            Err(_) => BakeOutcome::Failed("the bake worker panicked".into()),
        })
    }
}

impl Drop for LightmapBakeJob {
    /// A job dropped mid-bake (the editor closing) stops its worker rather than
    /// leaving it to burn the cores.
    fn drop(&mut self) {
        self.progress.cancel();
    }
}
