//! The occlusion bake's cost (#1714, part 3), measured by hand in a release build:
//! `cargo nextest run --release --features dev --run-ignored only -E
//! 'test(occlusion_bake_cost)' --no-capture`. A measurement, never a gate. Zero,
//! one and a Kolvik-sized level bake whole on 4 threads; the Dust2-sized one bakes
//! 40 cells spread over its grid on one thread and scales them up, which fits
//! nextest's five minutes (a whole bake took 705 s on 4 threads).

use std::time::Instant;

use glam::Vec3;

use rusty_lighting::occlusion::bench::level::rooms;
use rusty_lighting::occlusion::test_level::block;
use rusty_lighting::occlusion::{bake_occlusion, gather_occlusion};
use rusty_scene::Scene;

/// The process's peak resident memory, MB (Linux; 0 elsewhere).
fn peak_mb() -> u64 {
    let status = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    let line = status.lines().find(|l| l.starts_with("VmHWM:"));
    let kb = line.and_then(|l| l.split_whitespace().nth(1)?.parse::<u64>().ok());
    kb.unwrap_or(0) / 1024
}

#[test]
// lint: allow(skip) a timing measurement in a release build, run by hand, never a gate
#[ignore = "a release-build measurement, run by hand"]
fn occlusion_bake_cost() {
    let mut one = Scene::new();
    block(&mut one, "Box", Vec3::ZERO, Vec3::ONE);
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
    let input = gather_occlusion(&rooms(20));
    let t = Instant::now();
    let (cells, edge, sheets) = input.bake_sample(40).expect("a grid");
    let per = t.elapsed().as_secs_f64() / 40.0;
    println!(
        "Dust2-sized: {cells} cells of {edge:.2} m, sheets {sheets:?} (count, bytes), \
         {per:.3} s a cell on one thread, ~{:.0} s on 4, peak {} MB",
        per * cells as f64 / 4.0,
        peak_mb()
    );
}
