//! How far an occlusion bake has got (#1714): what the editor's bar and a
//! script's bake log read while a worker bakes.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

/// How far an occlusion bake has got (cells baked of the grid's), and the switch
/// that stops it. Plain atomics, read from another thread; neither touches what a
/// cell samples, so a bake run to the end is the same bytes with or without one.
#[derive(Debug, Default)]
pub struct OcclusionProgress {
    done: AtomicUsize,
    total: AtomicUsize,
    cancelled: AtomicBool,
}

impl OcclusionProgress {
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
    }

    pub(super) fn cell_done(&self) {
        self.done.fetch_add(1, Ordering::Relaxed);
    }
}
