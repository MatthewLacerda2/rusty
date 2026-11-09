//! A lightmap bake on a worker thread (#808), for the editor's Bake Lightmaps
//! button: the window stays live while the CPU path tracer runs.
//!
//! The editor's bake is progressive (#1970, Unity's Progressive Lightmapper,
//! `lightmap::bake_progressive`): after each pass the worker hands the lightmaps
//! as they stand to the preview writer beside it (`encoder::Encoder`) and traces
//! on, and [`LightmapBakeJob::show_preview`], called each frame, shows the newest
//! written on the live scene, what the Scene view sees converging first when the
//! settings' Prioritize View is on. The finished bake's bytes are the
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

use super::encoder::Encoder;
use super::preview::Previews;
use super::UNSAVED;
use super::{
    apply_encoded, encode_pages, gather_bake_input, lend_denoiser, lightmap_receivers,
    warn_missing_uvs, EncodedPages,
};
use crate::dev::bake::progress::{duration, eta, Progress, Step};
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
    /// The passes shown on the live scene so far.
    previews: Previews,
    /// When the first pass was seen done, and the texels traced by then: the
    /// time left is paced from there, past the first pass's validity probes.
    paced_from: Option<(Instant, u64)>,
}

impl LightmapBakeJob {
    /// Gather `scene` and start baking it progressively on a worker, what `view`
    /// sees converging first (#1970). Errors, without starting, when the
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
        let watch = Arc::clone(&progress);
        let view = view.filter(|_| settings.prioritize_view);
        let (page, taken) = (settings.page(), (lights, shadowmask));
        let mut encoder = Encoder::start(&previews, page, taken.clone(), Arc::clone(&progress))?;
        let worker = thread::Builder::new()
            .name("lightmap-bake".into())
            .spawn(move || {
                let baked = bake_progressive(&input, &settings, &watch, view, &mut encoder);
                // The last preview is written before the final pages are encoded.
                drop(encoder);
                match baked {
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
            paced_from: None,
        })
    }

    /// Show the newest finished pass on `scene` (#1970), once per pass, logging
    /// it; the pass shown, or `None` when no new one came in. A preview that cannot
    /// be written is said by the writer and skipped: the bake goes on.
    pub fn show_preview(&mut self, scene: &mut Scene) -> Option<Pass> {
        if self.paced_from.is_none() && self.passes().is_some_and(|p| p.done > 0) {
            self.paced_from = Some((Instant::now(), self.progress.done()));
        }
        let pass = self.previews.show(scene)?;
        log::info!("[LightmapBake] lightmaps {}", pass_line(pass, self.eta()));
        Some(pass)
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

    /// How long the trace still needs, at the pace it has gone since the first
    /// pass was done (`progress::eta`); `None` before then.
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
            Ok(Ok(Some(mut pages))) => {
                warn_missing_uvs(self.static_meshes, pages.maps);
                // An entity deleted while the bake ran gets no lightmap.
                pages.entries.retain(|e| scene.world.contains(e.entity));
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

/// The log's and the card's words for a pass: which, the paths per texel so far
/// (the view's too while it runs ahead), and the time left, as the headless bake
/// log says it (`progress::duration`): `pass 3/8 (24/64 samples), ~3 min 5 s
/// left`, or `pass 3/15 (8/64 samples, 24 in view)`. Before the first pass is
/// done, `first pass`.
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
