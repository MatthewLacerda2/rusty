//! The occlusion bake's cells timed on the platform's clock (#1987): the
//! [`OcclusionObserver`] a bake lends the sim. The sim only says where each cell
//! starts and ends; this side reads the clock, so no time value ever reaches the
//! bake. A cell runs on one worker, start to end, so its time is that worker's.
//!
//! One counter per cell: the grid is capped at [`MAX_GRID_CELLS`], so the clocks
//! hold at most 512 KiB, whatever the level. A cell the bake skips (#2328) is
//! never started, and reads as untimed.
//!
//! [`MAX_GRID_CELLS`]: rusty_scene::lighting::occlusion::MAX_GRID_CELLS

use std::cell::Cell;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::OnceLock;
use std::time::{Duration, Instant};

use rusty_lighting::occlusion::OcclusionObserver;

/// Each cell's sampling time so far, in nanoseconds, by cell index; [`UNTIMED`]
/// until it is sampled.
#[derive(Default)]
pub struct CellClocks {
    cells: OnceLock<Vec<AtomicU64>>,
}

/// A cell no worker has sampled.
const UNTIMED: u64 = u64::MAX;

thread_local! {
    /// When this thread's cell began: a cell starts and ends on one thread.
    static CELL_STARTED: Cell<Option<Instant>> = const { Cell::new(None) };
}

impl CellClocks {
    /// Each cell's time, by cell index, `None` for a cell never sampled; empty
    /// before the bake laid its grid.
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
