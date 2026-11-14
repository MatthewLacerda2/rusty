//! How far an occlusion bake has got (#1714): what the editor's bar and a
//! script's bake log read while a worker bakes.
//!
//! A progress made [`OcclusionProgress::observed`] also tells an
//! [`OcclusionObserver`] where each cell starts and ends (#1987), so the platform
//! can time the cells on its own clock. The calls return nothing, so nothing an
//! observer does can reach what a cell sees, and the sim never reads time.

use std::fmt;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

/// Who hears an occlusion bake's cells (#1987): the platform, which times them.
/// Every call returns nothing, so an observer can watch the bake but never steer
/// it. A cell starts and ends on the same thread, with no other cell between.
pub trait OcclusionObserver: Send + Sync {
    /// The bake laid a grid of `total` cells; once, before the first cell.
    fn cells(&self, total: usize);
    /// Cell `cell` begins sampling on this thread.
    fn cell_started(&self, cell: usize);
    /// That cell is sampled.
    fn cell_finished(&self, cell: usize);
}

/// The observer a progress was given, if any; `Debug` says only whether.
#[derive(Clone, Default)]
struct Observer(Option<Arc<dyn OcclusionObserver>>);

impl fmt::Debug for Observer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(if self.0.is_some() {
            "observed"
        } else {
            "unobserved"
        })
    }
}

/// How far an occlusion bake has got (cells baked of the grid's), and the switch
/// that stops it: plain atomics read from another thread, and the observer, if one
/// was lent, told where each cell starts and ends. None of them touches what a cell
/// samples, so a bake run to the end is the same bytes with or without one,
/// observed or not.
#[derive(Debug, Default)]
pub struct OcclusionProgress {
    done: AtomicUsize,
    total: AtomicUsize,
    cancelled: AtomicBool,
    observer: Observer,
}

impl OcclusionProgress {
    /// A progress that also reports each cell to `observer` (#1987): two calls
    /// per cell, none per ray.
    pub fn observed(observer: Arc<dyn OcclusionObserver>) -> Self {
        Self {
            observer: Observer(Some(observer)),
            ..Self::default()
        }
    }

    /// Cells baked, and the cells the bake has (0 until it laid its grid).
    pub fn cells(&self) -> (usize, usize) {
        let (done, total) = (&self.done, &self.total);
        (done.load(Ordering::Relaxed), total.load(Ordering::Relaxed))
    }

    /// Ask the bake to stop: it returns nothing within a cell per worker.
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }

    /// The bake laid a grid of `total` cells.
    pub(super) fn start(&self, total: usize) {
        self.total.store(total, Ordering::Relaxed);
        if let Some(o) = &self.observer.0 {
            o.cells(total);
        }
    }

    /// Cell `cell` begins sampling on this thread.
    pub(super) fn cell_started(&self, cell: usize) {
        if let Some(o) = &self.observer.0 {
            o.cell_started(cell);
        }
    }

    /// Cell `cell`, started on this thread, is sampled.
    pub(super) fn cell_done(&self, cell: usize) {
        if let Some(o) = &self.observer.0 {
            o.cell_finished(cell);
        }
        self.done.fetch_add(1, Ordering::Relaxed);
    }
}
