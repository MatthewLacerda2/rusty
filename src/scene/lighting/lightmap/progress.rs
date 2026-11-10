//! How far a lightmap bake has got, and the switch that stops it (#808).
//!
//! The bake reports into a [`BakeProgress`] it is lent; whoever lent it (the editor's
//! worker thread) reads the counters and may flip [`BakeProgress::cancel`]. Plain
//! atomics, so the sim depends on nothing above it. Neither ever touches a texel's
//! sample stream: a bake that ran to the end is byte-identical with or without one.
//!
//! A progress made [`BakeProgress::observed`] also tells a [`BakeObserver`] which
//! mesh each batch of texels belongs to as it starts and ends (#1891), so the
//! platform can time the meshes on its own clock. The calls return nothing, so
//! nothing an observer does can reach a texel, and the sim never reads time.
//!
//! A progress made [`BakeProgress::with_denoiser`] also lends the bake the trained
//! denoiser its `Oidn` charts go through (#1884): the platform owns the native
//! library, the sim only calls it.

use std::fmt;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

use super::bake::{LightmapDenoiser, Pass};

/// Who hears a lightmap bake's batches (#1891): the platform, which times them. Every
/// call returns nothing, so an observer can watch the bake but never steer it. A
/// batch's start and end come from the same thread, with no other batch between.
pub trait BakeObserver: Send + Sync {
    /// The meshes the bake traces, as (entity, texels), indexed by the `chart` the
    /// batch calls name; once, before the first batch.
    fn meshes(&self, meshes: &[(u32, u64)]);
    /// A batch of chart `chart`'s texels begins tracing on this thread.
    fn batch_started(&self, chart: usize);
    /// That batch is traced: `texels` of them.
    fn batch_finished(&self, chart: usize, texels: usize);
}

/// The observer a progress was given, if any; `Debug` says only whether.
#[derive(Clone, Default)]
struct Observer(Option<Arc<dyn BakeObserver>>);

impl fmt::Debug for Observer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(if self.0.is_some() {
            "observed"
        } else {
            "unobserved"
        })
    }
}

/// The trained denoiser a progress lends, if any; `Debug` says only whether.
#[derive(Clone, Default)]
struct Trained(Option<Arc<dyn LightmapDenoiser>>);

impl fmt::Debug for Trained {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(if self.0.is_some() { "lent" } else { "none" })
    }
}

/// Texels traced so far out of the bake's total, the meshes finished out of theirs
/// (#1667), the pages encoded out of theirs (#1694), a progressive bake's passes
/// and the paths per texel they have traced (#1970), plus a cancel request, and for an
/// observed one the [`BakeObserver`] its batches are reported to (#1891).
#[derive(Debug, Default)]
pub struct BakeProgress {
    done: AtomicU64,
    total: AtomicU64,
    meshes_done: AtomicU64,
    meshes_total: AtomicU64,
    pages_done: AtomicU64,
    pages_total: AtomicU64,
    passes_done: AtomicU64,
    passes_total: AtomicU64,
    samples_done: AtomicU64,
    view_done: AtomicU64,
    samples_total: AtomicU64,
    cancelled: AtomicBool,
    observer: Observer,
    denoiser: Trained,
}

impl BakeProgress {
    /// A progress that also reports each batch of texels to `observer` (#1891): two
    /// calls per batch, none per texel or ray.
    pub fn observed(observer: Arc<dyn BakeObserver>) -> Self {
        Self {
            observer: Observer(Some(observer)),
            ..Self::default()
        }
    }

    /// This progress, also lending the bake `denoiser` for its
    /// [`Denoiser::Oidn`](super::Denoiser::Oidn) charts (#1884). Without one they
    /// are filtered.
    pub fn with_denoiser(self, denoiser: Arc<dyn LightmapDenoiser>) -> Self {
        Self {
            denoiser: Trained(Some(denoiser)),
            ..self
        }
    }

    /// The denoiser lent, if any.
    pub(super) fn denoiser(&self) -> Option<&dyn LightmapDenoiser> {
        self.denoiser.0.as_deref()
    }

    /// Texels traced so far.
    pub fn done(&self) -> u64 {
        self.done.load(Ordering::Relaxed)
    }

    /// Texels the bake will trace; 0 until it has rasterized every mesh.
    pub fn total(&self) -> u64 {
        self.total.load(Ordering::Relaxed)
    }

    /// Meshes whose lightmap is finished.
    pub fn meshes_done(&self) -> u64 {
        self.meshes_done.load(Ordering::Relaxed)
    }

    /// Meshes the bake makes a lightmap for; 0 until it has rasterized every mesh.
    pub fn meshes_total(&self) -> u64 {
        self.meshes_total.load(Ordering::Relaxed)
    }

    /// Pages encoded so far and the pages the encode makes (#1694), once the trace
    /// is done and its encode has begun; `None` before.
    pub fn pages(&self) -> Option<(u64, u64)> {
        let total = self.pages_total.load(Ordering::Relaxed);
        (total > 0).then(|| (self.pages_done.load(Ordering::Relaxed), total))
    }

    /// A progressive bake's passes so far (#1970); `None` for a one-shot bake.
    pub fn passes(&self) -> Option<Pass> {
        let load = |n: &AtomicU64| n.load(Ordering::Relaxed) as u32;
        let passes = load(&self.passes_total);
        (passes > 0).then(|| Pass {
            done: load(&self.passes_done),
            passes,
            samples: load(&self.samples_done),
            view: load(&self.view_done),
            target: load(&self.samples_total),
        })
    }

    /// A progressive bake makes `total` steps, `samples` paths per texel in all.
    pub(super) fn set_passes(&self, total: u64, samples: u64) {
        self.samples_total.store(samples, Ordering::Relaxed);
        self.passes_total.store(total, Ordering::Relaxed);
    }

    /// A step is finished: `pass` says which, and the paths per texel so far.
    pub(super) fn pass_done(&self, pass: Pass) {
        self.samples_done
            .store(u64::from(pass.samples), Ordering::Relaxed);
        self.view_done
            .store(u64::from(pass.view), Ordering::Relaxed);
        self.passes_done
            .store(u64::from(pass.done), Ordering::Relaxed);
    }

    /// The encode has begun: it makes `total` pages (colour and direction alike).
    pub fn start_pages(&self, total: u64) {
        self.pages_done.store(0, Ordering::Relaxed);
        self.pages_total.store(total, Ordering::Relaxed);
    }

    /// One more page is encoded.
    pub fn page_done(&self) {
        self.pages_done.fetch_add(1, Ordering::Relaxed);
    }

    /// Ask the bake to stop. It notices within a few texels per worker, or between
    /// two levels of the encode, and returns nothing, so the caller keeps whatever
    /// lightmaps it had.
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }

    /// The texels and meshes the bake will trace, set once before the first texel.
    pub(super) fn set_total(&self, total: u64, meshes: u64) {
        self.meshes_total.store(meshes, Ordering::Relaxed);
        self.total.store(total, Ordering::Relaxed);
    }

    /// The meshes the bake traces, as (entity, texels), in its chart order; built
    /// only for an observer.
    pub(super) fn set_meshes(&self, meshes: impl Iterator<Item = (u32, u64)>) {
        if let Some(observer) = &self.observer.0 {
            observer.meshes(&meshes.collect::<Vec<_>>());
        }
    }

    pub(super) fn batch_started(&self, chart: usize) {
        if let Some(observer) = &self.observer.0 {
            observer.batch_started(chart);
        }
    }

    pub(super) fn batch_finished(&self, chart: usize, texels: usize) {
        if let Some(observer) = &self.observer.0 {
            observer.batch_finished(chart, texels);
        }
    }

    pub(super) fn mesh_done(&self) {
        self.meshes_done.fetch_add(1, Ordering::Relaxed);
    }

    pub(super) fn advance(&self, texels: u64) {
        self.done.fetch_add(texels, Ordering::Relaxed);
    }

    /// Take back `texels` the pool counted that were only looked at, never traced
    /// (Prioritize View's visibility check, #1970).
    /// A cancel mid-check counted fewer, so this stops at 0.
    pub(super) fn take_back(&self, texels: u64) {
        let less = |done: u64| Some(done.saturating_sub(texels));
        let _ = self
            .done
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, less);
    }
}
