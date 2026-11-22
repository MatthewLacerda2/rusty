//! A tile of n² cells sits only at a Z-order index that is a multiple of n², an aligned square;
//! the first free run wins, so placement is deterministic.

use super::plan::{CELLS, MIN_TILE};

#[derive(Clone, Debug)]
pub(super) struct CellGrid {
    used: Vec<bool>,
}

impl Default for CellGrid {
    fn default() -> Self {
        Self {
            used: vec![false; (CELLS * CELLS) as usize],
        }
    }
}

impl CellGrid {
    pub(super) fn place(&mut self, size: u32) -> Option<[u32; 2]> {
        let run = cells(size);
        let start = (0..CELLS * CELLS)
            .step_by(run as usize)
            .find(|&i| (i..i + run).all(|c| !self.used[c as usize]))?;
        self.mark(start, run, true);
        Some(cell_origin(start))
    }

    pub(super) fn free(&mut self, origin: [u32; 2], size: u32) {
        let run = cells(size);
        let start = cell_index(origin);
        debug_assert!(
            (start..start + run).all(|c| self.used[c as usize]),
            "freeing a tile the grid does not hold"
        );
        self.mark(start, run, false);
    }

    #[cfg(test)]
    pub(super) fn free_cells(&self) -> u32 {
        self.used.iter().filter(|u| !**u).count() as u32
    }

    fn mark(&mut self, start: u32, run: u32, used: bool) {
        for c in start..start + run {
            self.used[c as usize] = used;
        }
    }
}

pub(super) fn cells(size: u32) -> u32 {
    (size / MIN_TILE).pow(2)
}

/// Its even bits are x, its odd bits y.
fn cell_origin(index: u32) -> [u32; 2] {
    let compact = |mut v: u32| {
        let mut out = 0;
        for bit in 0..16 {
            out |= (v & 1) << bit;
            v >>= 2;
        }
        out
    };
    [compact(index) * MIN_TILE, compact(index >> 1) * MIN_TILE]
}

fn cell_index(origin: [u32; 2]) -> u32 {
    let spread = |v: u32| (0..16).fold(0, |out, bit| out | (((v >> bit) & 1) << (2 * bit)));
    let [x, y] = origin.map(|v| v / MIN_TILE);
    spread(x) | (spread(y) << 1)
}
