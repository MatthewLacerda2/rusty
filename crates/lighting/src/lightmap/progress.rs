//! None of this touches a texel's sample stream, so a bake that ran to the end is byte-identical with or without a progress; observer calls return nothing.

use std::fmt;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

use super::bake::{LightmapDenoiser, Pass};

pub trait BakeObserver: Send + Sync {
    /// As (entity, texels), indexed by the `chart` the batch calls name; once, before the first batch.
    fn meshes(&self, meshes: &[(u32, u64)]);
    fn batch_started(&self, chart: usize);
    fn batch_finished(&self, chart: usize, texels: usize);
    fn chart_finished(&self, _chart: usize) {}
}

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

#[derive(Clone, Default)]
struct Trained(Option<Arc<dyn LightmapDenoiser>>);

impl fmt::Debug for Trained {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(if self.0.is_some() { "lent" } else { "none" })
    }
}

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
    pub fn observed(observer: Arc<dyn BakeObserver>) -> Self {
        Self {
            observer: Observer(Some(observer)),
            ..Self::default()
        }
    }

    pub fn with_denoiser(self, denoiser: Arc<dyn LightmapDenoiser>) -> Self {
        Self {
            denoiser: Trained(Some(denoiser)),
            ..self
        }
    }

    pub(super) fn denoiser(&self) -> Option<&dyn LightmapDenoiser> {
        self.denoiser.0.as_deref()
    }

    pub fn done(&self) -> u64 {
        self.done.load(Ordering::Relaxed)
    }

    /// 0 until every mesh is rasterized.
    pub fn total(&self) -> u64 {
        self.total.load(Ordering::Relaxed)
    }

    pub fn meshes_done(&self) -> u64 {
        self.meshes_done.load(Ordering::Relaxed)
    }

    /// 0 until every mesh is rasterized.
    pub fn meshes_total(&self) -> u64 {
        self.meshes_total.load(Ordering::Relaxed)
    }

    pub fn pages(&self) -> Option<(u64, u64)> {
        let total = self.pages_total.load(Ordering::Relaxed);
        (total > 0).then(|| (self.pages_done.load(Ordering::Relaxed), total))
    }

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

    pub(super) fn set_passes(&self, total: u64, samples: u64) {
        self.samples_total.store(samples, Ordering::Relaxed);
        self.passes_total.store(total, Ordering::Relaxed);
    }

    pub(super) fn pass_done(&self, pass: Pass) {
        self.samples_done
            .store(u64::from(pass.samples), Ordering::Relaxed);
        self.view_done
            .store(u64::from(pass.view), Ordering::Relaxed);
        self.passes_done
            .store(u64::from(pass.done), Ordering::Relaxed);
    }

    pub fn start_pages(&self, total: u64) {
        self.pages_done.store(0, Ordering::Relaxed);
        self.pages_total.store(total, Ordering::Relaxed);
    }

    pub fn page_done(&self) {
        self.pages_done.fetch_add(1, Ordering::Relaxed);
    }

    /// Noticed within a few texels per worker, or between two encode levels.
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }

    pub(super) fn set_total(&self, total: u64, meshes: u64) {
        self.meshes_total.store(meshes, Ordering::Relaxed);
        self.total.store(total, Ordering::Relaxed);
    }

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

    pub(super) fn mesh_done(&self, chart: usize) {
        self.meshes_done.fetch_add(1, Ordering::Relaxed);
        if let Some(observer) = &self.observer.0 {
            observer.chart_finished(chart);
        }
    }

    pub(super) fn advance(&self, texels: u64) {
        self.done.fetch_add(texels, Ordering::Relaxed);
    }

    /// Texels the pool counted that were only looked at (Prioritize View); stops at 0 since a cancel mid-check counted fewer.
    pub(super) fn take_back(&self, texels: u64) {
        let less = |done: u64| Some(done.saturating_sub(texels));
        let _ = self
            .done
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, less);
    }
}
