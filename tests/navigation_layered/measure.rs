//! The CS:GO-sized level bakes into three stacked levels a path can climb through,
//! and (ignored by default) reports bake time and memory for the PR record.

use std::time::Instant;

use glam::Vec3;
use rusty::navigation::NavigationGraph;

use super::{bake, heights, level};

#[test]
fn level_bakes_three_floors_and_connects_street_to_roof() {
    let mut scene = level::build();
    let g = bake(&mut scene, level::BOUNDS);
    // Block (0,0): first floor over x = 14..38, roof over its east half.
    let floors: Vec<f32> = g.spans_at(32, 20).iter().map(|s| s.y).collect();
    assert_eq!(floors, vec![0.0, 4.0, 8.0], "street, first floor, roof");
    let path = g
        .path_between(Vec3::new(190.0, 0.0, 190.0), Vec3::new(30.0, 8.0, 15.0))
        .expect("street to roof via both ramps");
    let ys = heights(&g, &path);
    assert_eq!((ys[0], *ys.last().expect("path")), (0.0, 8.0), "{ys:?}");
}

/// `cargo test --release --features dev --test main -- --ignored --nocapture measure`
#[test]
#[ignore = "measurement for the PR record; run in release"]
fn measure_bake_time_and_memory() {
    for spacing in [1.0, 0.5, 0.25] {
        let mut scene = level::build();
        scene.nav_settings.grid_spacing = spacing;
        let start = Instant::now();
        let g = bake(&mut scene, level::BOUNDS);
        let elapsed = start.elapsed();
        report(spacing, elapsed.as_secs_f64() * 1e3, &g);
    }
    if let Ok(status) = std::fs::read_to_string("/proc/self/status") {
        let peak = status.lines().find(|l| l.starts_with("VmHWM"));
        eprintln!("process peak RSS: {}", peak.unwrap_or("n/a"));
    }
}

fn report(spacing: f32, ms: f64, g: &NavigationGraph) {
    let cells = (g.width * g.height) as usize;
    let bytes =
        std::mem::size_of_val(g.spans.as_slice()) + std::mem::size_of_val(g.cell_start.as_slice());
    eprintln!(
        "spacing {spacing}: {}x{} = {cells} cells, {} spans, {:.2} MiB, bake {ms:.1} ms",
        g.width,
        g.height,
        g.spans.len(),
        bytes as f64 / (1024.0 * 1024.0)
    );
}
