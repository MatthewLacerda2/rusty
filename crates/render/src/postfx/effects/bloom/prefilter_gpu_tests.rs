//! The prefilter's coverage on lavapipe (#1700), the shader's own taps where
//! `prefilter_tests` models them: a one-pixel lamp at every position inside a
//! first-mip texel's block reaches the chain at `÷3` and `÷4`, with close to the
//! same energy, dim or bright (where the Karis average damps it), so a spark moving
//! by single pixels doesn't switch its glow on and off.

use super::chain_gpu_tests::{bloom, half, halves, scene};
use rusty_components::Bloom;

/// The scene's side, and the cell each lamp sits in: both multiples of 3 and 4.
const SIDE: u32 = 96;
const CELL: u32 = 24;
/// Half-float bits and values: 0.0625, dim enough that the Karis weights stay near
/// linear, and 40.0, which they damp.
const DIM: (u16, f32) = (0x2c00, 0.0625);
const BRIGHT: (u16, f32) = (0x5100, 40.0);

/// Each lamp's energy in the chain's first mip at `divisor`, times `divisor²` over
/// its value (1: its exact share), one lamp of `(bits, value)` per position.
fn shares(r: &crate::Renderer, divisor: u32, (bits, value): (u16, f32)) -> Vec<f32> {
    // No threshold: the prefilter passes the light through as it filtered it.
    let knobs = Bloom {
        threshold: 0.0,
        ..Bloom::default()
    };
    // Lamp k in cell (k % 4, k / 4), at phase (k % d, k / d) of a texel's block.
    let lamp = |k: u32| {
        let cell = (k % 4 * CELL + 8, k / 4 * CELL + 8);
        ((cell.0 + k % divisor, cell.1 + k / divisor), bits)
    };
    let lamps: Vec<_> = (0..divisor * divisor).map(lamp).collect();
    let input = scene(r, SIDE, &lamps);
    let t = bloom(r, &input, SIDE, divisor, &knobs);
    let texels = halves(r, &t.down[0], t.size);
    let (side, cell) = (t.size.0, CELL / divisor);
    let share = |k: u32| {
        let (cx, cy) = (k % 4 * cell, k / 4 * cell);
        let energy: f32 = (cy..cy + cell)
            .flat_map(|y| (cx..cx + cell).map(move |x| (x, y)))
            .map(|(x, y)| half(texels[((y * side + x) * 4) as usize]))
            .sum();
        energy * (divisor * divisor) as f32 / value
    };
    (0..divisor * divisor).map(share).collect()
}

#[test]
fn gpu_a_one_pixel_lamp_reaches_the_chain_at_every_position() {
    let Some(r) = crate::test_gpu::headless_or_skip(SIDE, SIDE) else {
        return;
    };
    for divisor in [3u32, 4] {
        let dim = shares(&r, divisor, DIM);
        let bright = shares(&r, divisor, BRIGHT);
        eprintln!("[bloom] ÷{divisor} dim {dim:.3?} bright {bright:.3?}");
        let in_range = dim.iter().all(|s| (0.85..1.15).contains(s));
        assert!(in_range, "÷{divisor} dim: {dim:.3?}");
        let (lo, hi) = (
            bright.iter().cloned().fold(f32::MAX, f32::min),
            bright.iter().cloned().fold(0.0, f32::max),
        );
        assert!(lo > 0.0 && hi / lo < 1.3, "÷{divisor} bright: {bright:.3?}");
    }
}
