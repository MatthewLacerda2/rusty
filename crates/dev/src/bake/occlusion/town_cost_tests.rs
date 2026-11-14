//! The occlusion bake's cost on a town (#1981), measured by hand in a release
//! build: `cargo nextest run --release --features dev --run-ignored only -E
//! 'test(occlusion_town_cost)' --no-capture`. A measurement, never a gate.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::Instant;

use glam::Vec3;
use rusty_scene::lighting::occlusion::bench::level::rooms;

use rusty_scene::lighting::occlusion::bench::shift;
use rusty_scene::lighting::occlusion::bench::town::{town, Make};
use rusty_scene::lighting::occlusion::gather_occlusion;
use rusty_scene::Scene;

#[test]
// lint: allow(skip) a timing measurement in a release build, run by hand, never a gate
#[ignore = "a release-build measurement, run by hand"]
fn occlusion_town_cost() {
    let n = 6;
    measure("bench::level rooms(5)", &rooms(5));
    let configs = [
        (Make::Boxes, 0.0, 2),
        (Make::Boxes, 0.5, 2),
        (Make::Authored, 0.0, 2),
        (Make::Authored, 0.25, 2),
        (Make::Authored, 0.5, 2),
        (Make::Authored, 0.5, 8),
    ];
    for (make, open, props) in configs {
        let name = format!("{make:?} town, open {open:.2}, props {props}");
        measure(&name, &town(n, open, props, make));
    }
}

#[test]
// lint: allow(skip) a timing measurement in a release build, run by hand, never a gate
#[ignore = "a release-build measurement, run by hand"]
fn occlusion_town_scaling() {
    for (n, props) in [(4, 2), (6, 2), (8, 2), (6, 8), (6, 24), (8, 24)] {
        let name = format!("Authored town {n} x {n}, open 0.25, props {props}");
        measure(&name, &town(n, 0.25, props, Make::Authored));
    }
}

#[test]
// lint: allow(skip) a timing measurement in a release build, run by hand, never a gate
#[ignore = "a release-build measurement, run by hand"]
fn occlusion_town_shifted() {
    for make in [Make::Boxes, Make::Authored] {
        let mut level = town(6, 0.25, 2, make);
        shift(&mut level, Vec3::new(0.137, 0.053, -0.211), 1.07);
        measure(&format!("{make:?} town, open 0.25, shifted"), &level);
    }
}

/// Bake `level`'s cells on 4 threads, timing each, and print the rate, the
/// spread and a hash of what every cell sees.
fn measure(name: &str, level: &Scene) {
    let input = gather_occlusion(level);
    let t = Instant::now();
    let scene = input.sampler().expect("a grid");
    let build = t.elapsed().as_secs_f64();
    let count = scene.grid.cell_count();
    let (next, times) = (
        AtomicUsize::new(0),
        Mutex::new(vec![(0f64, Vec::new()); count]),
    );
    let t = Instant::now();
    std::thread::scope(|s| {
        for _ in 0..4 {
            s.spawn(|| loop {
                let cell = next.fetch_add(1, Ordering::Relaxed);
                if cell >= count {
                    break;
                }
                let c = Instant::now();
                let bits = scene.cell(cell);
                times.lock().unwrap()[cell] = (c.elapsed().as_secs_f64(), bits);
            });
        }
    });
    let secs = t.elapsed().as_secs_f64();
    let cells = times.into_inner().unwrap();
    let seen = cells.iter().flat_map(|c| &c.1);
    let hash = seen.fold(0xcbf2_9ce4_8422_2325u64, |h, w| {
        (h ^ w).wrapping_mul(0x100_0000_01b3)
    });
    let mut times: Vec<f64> = cells.iter().map(|c| c.0).collect();
    times.sort_by(f64::total_cmp);
    let total: f64 = times.iter().sum();
    let top: f64 = times[count - count / 100..].iter().sum();
    let rays = scene.rays_cast();
    println!(
        "{name}: {} meshes, {count} cells, build {build:.2} s, \
         bake {secs:.1} s = {:.0} cells/s, {:.0} rays/cell, median {:.1} ms, \
         p99 {:.1} ms, max {:.0} ms, slowest 1% = {:.0}% of the time, bits {hash:016x}",
        level.world.ids_with_mesh().len(),
        count as f64 / secs,
        rays as f64 / count as f64,
        times[count / 2] * 1e3,
        times[count * 99 / 100] * 1e3,
        times[count - 1] * 1e3,
        100.0 * top / total,
    );
}
