//! How far a lightmap bake has got, and the switch that stops it (#808).
//!
//! The bake reports into a [`BakeProgress`] it is lent; whoever lent it (the editor's
//! worker thread) reads the counters and may flip [`BakeProgress::cancel`]. Plain
//! atomics, so the sim depends on nothing above it. Neither ever touches a texel's
//! sample stream: a bake that ran to the end is byte-identical with or without one.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

/// Texels traced so far out of the bake's total, plus a cancel request.
#[derive(Debug, Default)]
pub struct BakeProgress {
    done: AtomicU64,
    total: AtomicU64,
    cancelled: AtomicBool,
}

impl BakeProgress {
    /// Texels traced so far.
    pub fn done(&self) -> u64 {
        self.done.load(Ordering::Relaxed)
    }

    /// Texels the bake will trace; 0 until it has rasterized every mesh.
    pub fn total(&self) -> u64 {
        self.total.load(Ordering::Relaxed)
    }

    /// Ask the bake to stop. It notices within a few texels per worker and returns
    /// nothing, so the caller keeps whatever lightmaps it had.
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }

    pub(super) fn set_total(&self, total: u64) {
        self.total.store(total, Ordering::Relaxed);
    }

    pub(super) fn advance(&self, texels: u64) {
        self.done.fetch_add(texels, Ordering::Relaxed);
    }
}
