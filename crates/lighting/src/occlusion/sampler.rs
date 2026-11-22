use super::sample::Baking;
use rusty_scene::lighting::occlusion::Grid;

/// A test hook: one bake's sampling, which `dev::bake`'s cost tests time.
#[doc(hidden)]
pub struct Sampler {
    pub(super) baking: Baking,
    pub grid: Grid,
}

impl Sampler {
    pub fn cell(&self, index: usize) -> Vec<u64> {
        self.baking.cell(&self.grid, index)
    }

    pub fn spread(&self, count: u64) {
        let cells = self.grid.cell_count() as u64;
        for k in 0..count {
            let cell = k.wrapping_mul(0x9E37_79B9_7F4A_7C15) % cells;
            std::hint::black_box(self.cell(cell as usize));
        }
    }

    pub fn rays_cast(&self) -> u64 {
        self.baking.rays_cast()
    }
}
