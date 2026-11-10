//! The navmesh bake's cost (#1887), measured by hand in a release build:
//! `cargo nextest run --release --features dev --run-ignored only -E
//! 'test(navmesh_bake_cost)' --no-capture`. A measurement, never a gate. The
//! Dust2-sized level is the occlusion bench's (`bench::level::rooms(20)`, 200 ×
//! 200 m on two storeys) with a box collider on every block; it bakes whole at the
//! old 1 m cell and the 0.25 m default, then a crate moves and the incremental
//! rebake is timed against the full one. On 2026-11-08, in a 4-core container:
//! 1 m bakes in 0.08 s (60k spans, 1.9 MB), 0.25 m in 1.09 s (1.37M spans, 41 MB,
//! twice that with the pre-erosion copy the incremental rebake keeps).

use std::time::Instant;

use glam::Vec3;

use crate::components::{ColliderComponent, ColliderShape};
use crate::navigation::{NavigationGraph, Rebake};
use crate::scene::lighting::occlusion::bench::level::rooms;
use crate::scene::Scene;

/// The process's peak resident memory, MB (Linux; 0 elsewhere).
fn peak_mb() -> u64 {
    let status = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    let line = status.lines().find(|l| l.starts_with("VmHWM:"));
    let kb = line.and_then(|l| l.split_whitespace().nth(1)?.parse::<u64>().ok());
    kb.unwrap_or(0) / 1024
}

/// `rooms(n)` with a unit box collider on every block (each is a scaled unit box).
pub(super) fn level(n: usize, spacing: f32) -> Scene {
    let mut scene = rooms(n);
    scene.nav_settings.grid_spacing = spacing;
    let ids: Vec<u32> = scene.world.ids().to_vec();
    for id in ids {
        let mut c = ColliderComponent::from_mesh_bounds(false, Vec3::ZERO, Vec3::ZERO);
        c.shape = ColliderShape::Box { size: Vec3::ONE };
        scene.world.set_collider(id, Some(c));
    }
    scene
}

#[test]
// lint: allow(skip) a timing measurement in a release build, run by hand, never a gate
#[ignore = "a release-build measurement, run by hand"]
fn navmesh_bake_cost() {
    for spacing in [1.0, 0.25] {
        let mut scene = level(20, spacing);
        let mut g = NavigationGraph::new(0.0, 1.0, 0.0, 1.0, 1.0);
        let t = Instant::now();
        g.bake(&scene);
        let full = t.elapsed().as_secs_f64();
        let spans = g.spans.len();
        let mb = (g.spans.len() * std::mem::size_of::<crate::navigation::NavSpan>()
            + g.cell_start.len() * 4) as f64
            / 1e6;
        let bytes = NavigationGraph::new(0.0, 1.0, 0.0, 1.0, 1.0).bake_to_bytes(&scene);
        let t = Instant::now();
        let mut restored = NavigationGraph::new(0.0, 1.0, 0.0, 1.0, 1.0);
        restored
            .restore(&scene, &bytes)
            .expect("a fresh file restores");
        let load = t.elapsed().as_secs_f64();
        let crates: Vec<u32> = (scene.world.ids().iter().copied())
            .filter(|&id| scene.world.name(id).is_some_and(|n| *n == "Crate"))
            .collect();
        let crate_id = crates[crates.len() / 2];
        if let Some(mut tr) = scene.world.transform_mut(crate_id) {
            tr.position.x += 0.5;
        }
        let t = Instant::now();
        let rebake = g.sync(&scene);
        let inc = t.elapsed().as_secs_f64();
        let cells: i64 = match &rebake {
            Rebake::Incremental(rs) => rs.iter().map(|r| r.cells() as i64).sum(),
            _ => -1,
        };
        println!(
            "{spacing} m: {}x{} cells, {spans} spans, full bake {full:.2} s, graph {mb:.1} MB \
             (x2 with the pre-erosion copy), peak {} MB; a moved block rebakes {cells} cells \
             in {inc:.3} s; the saved file is {:.1} MB and restores in {load:.2} s",
            g.width,
            g.height,
            peak_mb(),
            bytes.len() as f64 / 1e6
        );
    }
}
