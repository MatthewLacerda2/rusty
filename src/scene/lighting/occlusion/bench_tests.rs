//! The occlusion bake's timing on the Dust2-sized level (#1714, part 3), run by
//! hand in a release build: `cargo nextest run --release --run-ignored only -E
//! 'test(occlusion_bake_timing)' --no-capture`. A measurement, never a gate.

use std::time::Instant;

use super::baking::Baking;
use super::bench_level::rooms;
use super::{gather_occlusion, Grid};

#[test]
#[ignore = "a release-build measurement, run by hand"]
fn occlusion_bake_timing() {
    for n in [5usize, 20] {
        let scene = rooms(n);
        let t = Instant::now();
        let input = gather_occlusion(&scene);
        let objects = input.objects();
        let baking = Baking::of(objects);
        let prep = t.elapsed().as_secs_f64();
        let (min, max) = baking.bounds.expect("cullable");
        let grid = Grid::over(min, max).expect("fits");
        let count = grid.cell_count();
        let stride = (count / 200).max(1);
        let t = Instant::now();
        let mut sampled = 0;
        for cell in (0..count).step_by(stride) {
            std::hint::black_box(baking.cell(&grid, cell));
            sampled += 1;
        }
        let per = t.elapsed().as_secs_f64() / sampled as f64;
        println!(
            "n={n}: {} objects, {count} cells of {:.2} m, prep {prep:.2} s, {:.3} s a cell, ~{:.0} s on 4 threads",
            objects.len(), grid.cell, per, per * count as f64 / 4.0
        );
    }
}
