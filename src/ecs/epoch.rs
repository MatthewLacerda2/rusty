//! src/ecs/epoch.rs — a token that changes whenever an id stops naming the same entity (#727).
//!
//! Caches keyed by stable id (the scene's world-matrix cache) outlive the frame they
//! were filled in, and an id can stop meaning what it meant then: a despawn ends its
//! entity, and a `clear` (scene load) restarts the allocator at 1, so the next spawn
//! reuses it. [`World`](super::World) draws a fresh epoch at each of those, and a
//! cache that stamps its fill with the epoch treats any mismatch as "rebuild", so it
//! never answers for an entity that is gone.
//!
//! Epochs come from one process-wide counter, so two worlds never share one either.
//! They only gate a cache, never feed a result, so they have no bearing on determinism.

use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Epoch(u64);

impl Epoch {
    /// An epoch no world has had before.
    pub(super) fn fresh() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        Epoch(NEXT.fetch_add(1, Ordering::Relaxed))
    }
}
