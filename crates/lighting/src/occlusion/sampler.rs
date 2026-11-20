//! The occlusion bake's test hook for a cost measurement (#1987): its sampling, a
//! cell at a time, from another crate.

use super::sample::Baking;
use rusty_scene::lighting::occlusion::Grid;

/// One bake's sampling, a cell at a time ([`OcclusionInput::sampler`]). A test
/// hook: `dev::bake`'s cost tests measure through it, from another crate.
#[doc(hidden)]
pub struct Sampler {
    pub(super) baking: Baking,
    pub grid: Grid,
}

impl Sampler {
    /// What cell `index` sees on its own, as the bake samples it.
    pub fn cell(&self, index: usize) -> Vec<u64> {
        self.baking.cell(&self.grid, index)
    }

    /// Bake `count` cells spread over the grid, one after another, on this
    /// thread: the same cells for the same level and count.
    pub fn spread(&self, count: u64) {
        let cells = self.grid.cell_count() as u64;
        for k in 0..count {
            let cell = k.wrapping_mul(0x9E37_79B9_7F4A_7C15) % cells;
            std::hint::black_box(self.cell(cell as usize));
        }
    }

    /// Rays cast so far, either kind.
    pub fn rays_cast(&self) -> u64 {
        self.baking.rays_cast()
    }
}
