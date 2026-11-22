use std::cell::Cell;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use rusty_lighting::occlusion::OcclusionObserver;

#[derive(Default)]
pub struct CellClocks {
    cells: OnceLock<Vec<AtomicU64>>,
}

const UNTIMED: u64 = u64::MAX;

// Per thread: a cell starts and ends on one worker.
thread_local! {
    static CELL_STARTED: Cell<Option<Instant>> = const { Cell::new(None) };
}

impl CellClocks {
    pub fn times(&self) -> Vec<Option<Duration>> {
        let cells = self.cells.get().map_or(&[][..], Vec::as_slice);
        let time = |nanos: &AtomicU64| match nanos.load(Ordering::Relaxed) {
            UNTIMED => None,
            n => Some(Duration::from_nanos(n)),
        };
        cells.iter().map(time).collect()
    }
}

impl OcclusionObserver for CellClocks {
    fn cells(&self, total: usize) {
        let _ = self
            .cells
            .set((0..total).map(|_| AtomicU64::new(UNTIMED)).collect());
    }

    fn cell_started(&self, _cell: usize) {
        CELL_STARTED.set(Some(Instant::now()));
    }

    fn cell_finished(&self, cell: usize) {
        let Some(from) = CELL_STARTED.take() else {
            return;
        };
        if let Some(nanos) = self.cells.get().and_then(|c| c.get(cell)) {
            let took = (from.elapsed().as_nanos() as u64).min(UNTIMED - 1);
            let add = |n: u64| Some(if n == UNTIMED { took } else { n + took });
            let _ = nanos.fetch_update(Ordering::Relaxed, Ordering::Relaxed, add);
        }
    }
}
