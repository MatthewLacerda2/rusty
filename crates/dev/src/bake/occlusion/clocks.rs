//! The occlusion bake's cells timed on the platform's clock (#1987): the
//! [`OcclusionObserver`] a bake lends the sim. The sim only says where each cell
//! starts and ends; this side reads the clock, so no time value ever reaches the
//! bake. A cell runs on one worker, start to end, so its time is that worker's.
//!
//! One counter per cell: the grid is capped at [`MAX_CELLS`], so the clocks hold
//! at most 128 KiB, whatever the level.
//!
//! [`MAX_CELLS`]: rusty_scene::lighting::occlusion::MAX_CELLS

use std::cell::Cell;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use rusty_lighting::occlusion::OcclusionObserver;

/// Each cell's sampling time so far, in nanoseconds, by cell index.
#[derive(Default)]
pub struct CellClocks {
    cells: OnceLock<Vec<AtomicU64>>,
}

thread_local! {
    /// When this thread's cell began: a cell starts and ends on one thread.
    static CELL_STARTED: Cell<Option<Instant>> = const { Cell::new(None) };
}

impl CellClocks {
    /// Each cell's time, by cell index; empty before the bake laid its grid.
    pub fn times(&self) -> Vec<Duration> {
        let cells = self.cells.get().map_or(&[][..], Vec::as_slice);
        let time = |nanos: &AtomicU64| Duration::from_nanos(nanos.load(Ordering::Relaxed));
        cells.iter().map(time).collect()
    }
}

impl OcclusionObserver for CellClocks {
    fn cells(&self, total: usize) {
        let _ = self
            .cells
            .set((0..total).map(|_| AtomicU64::new(0)).collect());
    }

    fn cell_started(&self, _cell: usize) {
        CELL_STARTED.set(Some(Instant::now()));
    }

    fn cell_finished(&self, cell: usize) {
        let Some(from) = CELL_STARTED.take() else {
            return;
        };
        if let Some(nanos) = self.cells.get().and_then(|c| c.get(cell)) {
            nanos.fetch_add(from.elapsed().as_nanos() as u64, Ordering::Relaxed);
        }
    }
}
