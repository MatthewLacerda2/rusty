//! The lightmap bake's batches timed on the platform's clock (#1891): the
//! [`BakeObserver`] a logged bake lends the sim. The sim only says which mesh a batch
//! belongs to as it starts and ends; this side reads the clock and adds each batch's
//! time to its mesh, so no time value ever reaches the bake.
//!
//! The pool (#1656) traces one mesh's batches on every core at once, so a mesh's time
//! is its worker time, summed over the threads, not a wall-clock span.

use std::cell::Cell;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use crate::scene::lighting::lightmap::BakeObserver;

/// One mesh's share of a timed bake: its entity, its texels, and the worker time its
/// batches took, summed over every thread that traced one.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MeshTime {
    pub entity: u32,
    pub texels: u64,
    pub busy: Duration,
}

/// Each mesh's worker time so far, as (entity, texels, nanoseconds).
#[derive(Default)]
pub struct MeshClocks {
    meshes: OnceLock<Vec<(u32, u64, AtomicU64)>>,
}

thread_local! {
    /// When this thread's batch began: a batch starts and ends on one thread.
    static BATCH_STARTED: Cell<Option<Instant>> = const { Cell::new(None) };
}

impl MeshClocks {
    /// Each mesh's worker time, in bake order; empty before the bake named its meshes.
    pub fn times(&self) -> Vec<MeshTime> {
        let meshes = self.meshes.get().map_or(&[][..], Vec::as_slice);
        let time = |(entity, texels, nanos): &(u32, u64, AtomicU64)| MeshTime {
            entity: *entity,
            texels: *texels,
            busy: Duration::from_nanos(nanos.load(Ordering::Relaxed)),
        };
        meshes.iter().map(time).collect()
    }
}

impl BakeObserver for MeshClocks {
    fn meshes(&self, meshes: &[(u32, u64)]) {
        let clocks = meshes.iter().map(|&(e, t)| (e, t, AtomicU64::new(0)));
        let _ = self.meshes.set(clocks.collect());
    }

    fn batch_started(&self, _chart: usize) {
        BATCH_STARTED.set(Some(Instant::now()));
    }

    fn batch_finished(&self, chart: usize, _texels: usize) {
        let Some(from) = BATCH_STARTED.take() else {
            return;
        };
        if let Some((_, _, nanos)) = self.meshes.get().and_then(|m| m.get(chart)) {
            nanos.fetch_add(from.elapsed().as_nanos() as u64, Ordering::Relaxed);
        }
    }
}
