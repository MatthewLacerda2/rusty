use std::cell::Cell;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use rusty_lighting::lightmap::BakeObserver;

/// `busy` is worker time summed over threads: the pool traces one mesh on every core at once.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MeshTime {
    pub entity: u32,
    pub texels: u64,
    pub busy: Duration,
}

#[derive(Default)]
pub struct MeshClocks {
    meshes: OnceLock<Vec<(u32, u64, AtomicU64)>>,
}

// Per thread: a batch starts and ends on one thread.
thread_local! {
    static BATCH_STARTED: Cell<Option<Instant>> = const { Cell::new(None) };
}

impl MeshClocks {
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

    fn chart_finished(&self, _chart: usize) {
        crate::memory::relieve();
    }
}
