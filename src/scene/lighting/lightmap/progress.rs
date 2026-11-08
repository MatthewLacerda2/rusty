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

use std::fmt;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

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

/// Texels traced so far out of the bake's total, the meshes finished out of theirs
/// (#1667), the pages encoded out of theirs (#1694), plus a cancel request, and for an
/// observed one the [`BakeObserver`] its batches are reported to (#1891).
#[derive(Debug, Default)]
pub struct BakeProgress {
    done: AtomicU64,
    total: AtomicU64,
    meshes_done: AtomicU64,
    meshes_total: AtomicU64,
    pages_done: AtomicU64,
    pages_total: AtomicU64,
    cancelled: AtomicBool,
    observer: Observer,
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
}
