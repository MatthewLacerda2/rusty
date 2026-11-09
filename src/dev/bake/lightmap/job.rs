//! A lightmap bake on a worker thread (#808), for the editor's Bake Lightmaps
//! button: the window stays live while the CPU path tracer runs.
//!
//! The editor's bake is progressive (#1970, Unity's Progressive Lightmapper,
//! `lightmap::bake_progressive`): after each pass the worker encodes the lightmaps
//! as they stand, and [`LightmapBakeJob::show_preview`], called each frame, shows
//! them on the live scene (`preview`), the batches the Scene view sees traced first
//! when the settings' Prioritize View is on. The finished bake's bytes are the
//! one-shot bake's, so only what is shown on the way differs from a script's bake.
//!
//! [`LightmapBakeJob::start`] gathers the owned `BakeScene` on the caller's thread
//! (it borrows the ECS) and hands it to a worker, which bakes and then encodes the
//! pages (`encode_pages`: BC6H and every mip, and the compressed PNG, #1694), so
//! neither blocks a frame; [`LightmapBakeJob::poll`], called each frame, writes the
//! finished bytes and points the scene at them on the caller's thread, through
//! the same [`apply_encoded`] the synchronous `Lighting.BakeLightmaps()` uses. A
//! cancelled bake applies nothing, so the scene keeps its previous lightmaps: a
//! cancel lands during the trace, between two levels of the encode, or after it,
//! up to the `poll` that would have applied it.

use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use super::preview::Previews;
use super::UNSAVED;
use super::{
    apply_encoded, encode_pages, gather_bake_input, lend_denoiser, lightmap_receivers,
    warn_missing_uvs, EncodedPages,
};
use crate::scene::lighting::lightmap::{
    bake_progressive, BakeProgress, BakeSettings, BakeView, MaskChannels, Pass,
};
use crate::scene::Scene;

/// What the worker hands back: the encoded pages, `None` when cancelled, or why
/// encoding failed.
type Encoded = Result<Option<EncodedPages>, String>;

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
    /// The passes shown on the live scene so far, and the lightmaps they hide.
    previews: Previews,
    started: Instant,
}

impl LightmapBakeJob {
    /// Gather `scene` and start baking it progressively on a worker, the batches
    /// `view` sees first in each pass (#1970). Errors, without starting, when the
    /// scene was never saved (there is nowhere to write the pages).
    pub fn start(
        scene: &mut Scene,
        scene_path: Option<&str>,
        settings: BakeSettings,
        view: Option<BakeView>,
    ) -> Result<Self, String> {
        let scene_path = scene_path.ok_or(UNSAVED)?.to_string();
        let input = gather_bake_input(scene);
        let lights = crate::scene::lighting::lightmap::taken_lights(scene);
        let shadowmask = input.shadowmask.clone();
        let static_meshes = lightmap_receivers(&input);
        let progress = Arc::new(lend_denoiser(BakeProgress::default(), &settings).0);
        let previews = Previews::new(&scene_path);
        let (watch, latest) = (Arc::clone(&progress), previews.latest());
        let view = view.filter(|_| settings.prioritize_view);
        let worker = thread::Builder::new()
            .name("lightmap-bake".into())
            .spawn(move || {
                let (page, taken) = (settings.page(), (lights, shadowmask));
                let mut on_pass = |pass: Pass, maps: &[_]| {
                    Previews::encode(&latest, (pass, maps), page, &taken, &watch)
                };
                match bake_progressive(&input, &settings, &watch, view, &mut on_pass) {
                    Some(maps) => encode_pages(&maps, page, &watch)
                        .map(|pages| pages.map(|p| labelled(p, &taken))),
                    None => Ok(None),
                }
            })
            .map_err(|e| format!("start the bake: {e}"))?;
        Ok(Self {
            progress,
            worker: Some(worker),
            scene_path,
            static_meshes,
            previews,
            started: Instant::now(),
        })
    }

    /// Show the newest finished pass on `scene` (#1970), once per pass, logging
    /// it; the pass shown, or `None` when no new one came in. A preview that cannot
    /// be written is said and skipped: the bake goes on.
    pub fn show_preview(&mut self, scene: &mut Scene) -> Option<Pass> {
        match self.previews.show(scene) {
            Ok(Some(pass)) => {
                log::info!("[LightmapBake] {}", pass_line(pass, self.eta()));
                Some(pass)
            }
            Ok(None) => None,
            Err(e) => {
                log::warn!("[LightmapBake] show a preview pass: {e}");
                None
            }
        }
    }

    /// Put back the lightmaps a preview is shown over (a bake dropped while its
    /// scene stays open), and remove the preview pages.
    pub fn restore(&mut self, scene: &mut Scene) {
        self.previews.restore(scene);
    }

    /// The progressive bake's passes so far (#1970); `None` until it has rasterized.
    pub fn passes(&self) -> Option<Pass> {
        self.progress.passes()
    }

    /// How long the trace still needs, at the pace it has gone so far (from the
    /// texels traced over every pass); `None` before the first texel.
    pub fn eta(&self) -> Option<Duration> {
        let (done, total) = self.progress();
        (done > 0 && total >= done).then(|| {
            let left = (total - done) as f64 / done as f64;
            self.started.elapsed().mul_f64(left)
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
            Ok(Ok(Some(_))) if self.progress.is_cancelled() => BakeOutcome::Cancelled,
            Ok(Ok(Some(pages))) => {
                warn_missing_uvs(self.static_meshes, pages.maps);
                match apply_encoded(scene, &self.scene_path, &pages) {
                    Ok(n) => BakeOutcome::Baked(n),
                    Err(e) => BakeOutcome::Failed(e),
                }
            }
            Ok(Ok(None)) => BakeOutcome::Cancelled,
            Ok(Err(e)) => BakeOutcome::Failed(e),
            Err(_) => BakeOutcome::Failed("the bake worker panicked".into()),
        })
        .inspect(|outcome| {
            // Only a finished bake is written (Unity's Cancel): anything else puts
            // the scene's own lightmaps back from under the preview.
            if !matches!(outcome, BakeOutcome::Baked(_)) {
                self.previews.restore(scene);
            }
        })
    }
}

impl Drop for LightmapBakeJob {
    /// A job dropped mid-bake (the editor closing) stops its worker rather than
    /// leaving it to burn the cores.
    fn drop(&mut self) {
        self.progress.cancel();
        // Its scene is gone or restored; a finished bake already cleared them.
        self.previews.clear();
    }
}

/// The lights a bake took in and its shadowmask channels, set on its pages.
pub(super) type Taken = (std::collections::BTreeSet<u32>, MaskChannels);

/// `pages` with the lights and shadowmask channels the bake's input took in.
pub(super) fn labelled(pages: EncodedPages, (lights, shadowmask): &Taken) -> EncodedPages {
    EncodedPages {
        lights: lights.clone(),
        shadowmask: shadowmask.clone(),
        ..pages
    }
}

/// The log's and the card's line for a pass: which, the paths per texel so far,
/// and the time left.
pub fn pass_line(pass: Pass, eta: Option<Duration>) -> String {
    let left = eta.map_or(String::new(), |t| format!(", about {} left", clock(t)));
    format!(
        "lightmaps pass {}/{} ({}/{} samples){left}",
        pass.done, pass.passes, pass.samples, pass.target
    )
}

/// `t` as `1h 02m`, `3m 05s` or `12s`.
pub fn clock(t: Duration) -> String {
    let s = t.as_secs();
    match (s / 3600, s / 60 % 60, s % 60) {
        (0, 0, s) => format!("{s}s"),
        (0, m, s) => format!("{m}m {s:02}s"),
        (h, m, _) => format!("{h}h {m:02}m"),
    }
}
