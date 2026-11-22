use super::chain_gpu_tests::{bloom, half, halves, scene};
use rusty_components::Bloom;

const SIDE: u32 = 96;
const CELL: u32 = 24;
const DIM: (u16, f32) = (0x2c00, 0.0625);
const BRIGHT: (u16, f32) = (0x5100, 40.0);

fn shares(r: &crate::Renderer, divisor: u32, (bits, value): (u16, f32)) -> Vec<f32> {
    let knobs = Bloom {
        threshold: 0.0,
        ..Bloom::default()
    };
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
