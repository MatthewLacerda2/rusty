//! The occlusion bake's cost (#1714, part 3), measured by hand in a release build:
//! `cargo nextest run --release --run-ignored only -E 'test(occlusion_bake_cost)'
//! --no-capture`. A measurement, never a gate. Zero, one and a Kolvik-sized level
//! bake whole on 4 threads; the Dust2-sized one times 40 cells spread over its grid
//! and scales them up, which fits nextest's five minutes.

use std::time::Instant;

use super::baking::Baking;
use super::bench_level::rooms;
use super::test_level::block;
use super::{bake_occlusion, gather_occlusion, Grid};
use crate::scene::Scene;

/// The process's peak resident memory, MB (Linux; 0 elsewhere).
fn peak_mb() -> u64 {
    let status = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    let line = status.lines().find(|l| l.starts_with("VmHWM:"));
    let kb = line.and_then(|l| l.split_whitespace().nth(1)?.parse::<u64>().ok());
    kb.unwrap_or(0) / 1024
}

#[test]
#[ignore = "a release-build measurement, run by hand"]
fn occlusion_bake_cost() {
    let mut one = Scene::new();
    block(&mut one, "Box", glam::Vec3::ZERO, glam::Vec3::ONE);
    for (name, scene) in [
        ("zero", Scene::new()),
        ("one", one),
        ("Kolvik-sized", rooms(5)),
    ] {
        let t = Instant::now();
        let (_, report) = bake_occlusion(&scene, 4);
        let secs = t.elapsed().as_secs_f64();
        println!(
            "{name}: {secs:.2} s whole, {report:?}, peak {} MB",
            peak_mb()
        );
    }
    let scene = rooms(20);
    let t = Instant::now();
    let input = gather_occlusion(&scene);
    let baking = Baking::of(input.objects());
    let prep = t.elapsed().as_secs_f64();
    let (min, max) = baking.bounds.expect("cullable");
    let grid = Grid::over(min, max).expect("fits");
    let count = grid.cell_count();
    let t = Instant::now();
    for k in 0..40u64 {
        let cell = (k * 0x9E37_79B9_7F4A_7C15 % count as u64) as usize;
        std::hint::black_box(baking.cell(&grid, cell));
    }
    let per = t.elapsed().as_secs_f64() / 40.0;
    println!(
        "Dust2-sized: {} objects, {count} cells of {:.2} m, prep {prep:.2} s, {per:.3} s a cell on one thread, ~{:.0} s on 4, peak {} MB",
        input.objects().len(),
        grid.cell,
        per * count as f64 / 4.0,
        peak_mb()
    );
}
