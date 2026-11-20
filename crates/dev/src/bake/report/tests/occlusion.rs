//! The occlusion report: on the corridor test level the PVS culls a share of the
//! objects, the slowest cells come slowest first, and Bake Occlusion writes the
//! report beside the scene.

use std::time::Duration;

use super::{assert_common, scene_path};
use crate::bake::generate_lighting::bake_occlusion_only;
use crate::bake::progress::BakeLog;
use crate::bake::report::{occlusion, point, read, Kind, Stages, TOP};
use rusty_lighting::occlusion::bake_occlusion;
use rusty_lighting::occlusion::test_level::corridors;

#[test]
fn the_corridors_cull_a_share_of_their_objects() {
    let scene = corridors();
    let (data, r) = bake_occlusion(&scene, 2);
    let data = data.expect("the corridors occlude");
    let report = occlusion::report(&scene, (&r, Some(&data), &[]), &Stages::start(), 0).to_json();
    let v = &report["visibility"];
    let culled = v["culledFraction"].as_f64().unwrap();
    assert!(
        (0.2..1.0).contains(&culled),
        "the rooms hide each other: {v}"
    );
    let (mean, max) = (
        v["meanVisible"].as_f64().unwrap(),
        v["maxVisible"].as_f64().unwrap(),
    );
    assert!(mean <= max && max <= r.objects as f64, "{v}");
    let biggest = report["biggestOccluders"].as_array().unwrap();
    assert!(!biggest.is_empty() && report["occluderTriangles"].as_u64() > Some(0));
    assert!(
        report["slowestCells"].is_null() && report["cellSeconds"].is_null(),
        "a bake nobody timed"
    );
}

#[test]
fn the_slowest_cells_come_first_and_ties_go_to_the_lower_cell() {
    let scene = corridors();
    let (data, r) = bake_occlusion(&scene, 2);
    let data = data.expect("the corridors occlude");
    assert!(r.cells > TOP + 2, "enough cells to cut: {}", r.cells);
    // Cell 3 slowest, cell 1 next, then every other cell tied at 1 ms.
    let mut cells = vec![Duration::from_millis(1); r.cells];
    (cells[3], cells[1]) = (Duration::from_millis(9), Duration::from_millis(5));
    let report = occlusion::report(&scene, (&r, Some(&data), &cells), &Stages::start(), 0);
    let report = report.to_json();
    let slowest = report["slowestCells"].as_array().unwrap();
    let order: Vec<u64> = slowest
        .iter()
        .map(|c| c["cell"].as_u64().unwrap())
        .collect();
    assert_eq!(order.len(), TOP);
    assert_eq!(order[..4], [3, 1, 0, 2], "{order:?}");
    assert!(order[2..].windows(2).all(|w| w[0] < w[1]), "{order:?}");
    let (min, max) = data.grid.bounds(3);
    let centre = (min + max) * 0.5;
    assert_eq!(slowest[0]["centre"], point(centre));
    assert_eq!(slowest[0]["seconds"].as_f64(), Some(0.009));
    let visible = slowest[0]["visible"].as_u64().unwrap();
    assert!(visible > 0 && visible <= r.objects as u64, "{visible}");
    let seconds = &report["cellSeconds"];
    assert_eq!(seconds["max"].as_f64(), Some(0.009));
    let mean = (9 + 5 + (r.cells - 2)) as f64 / r.cells as f64 / 1000.0;
    assert!(
        (seconds["mean"].as_f64().unwrap() - mean).abs() < 2e-6,
        "{seconds}"
    );
    // Times for another grid are not this bake's.
    let other = occlusion::report(&scene, (&r, Some(&data), &cells[1..]), &Stages::start(), 0);
    assert!(other.to_json()["slowestCells"].is_null());
}

#[test]
fn each_grid_layer_sums_its_cells_times() {
    let scene = corridors();
    let (data, r) = bake_occlusion(&scene, 2);
    let data = data.expect("the corridors occlude");
    let g = data.grid;
    let per_layer = (g.dims.x * g.dims.z) as usize;
    // Every cell of layer `y` takes `y + 1` ms.
    let cells: Vec<_> = (0..r.cells)
        .map(|i| Duration::from_millis((i / per_layer) as u64 + 1))
        .collect();
    let report = occlusion::report(&scene, (&r, Some(&data), &cells), &Stages::start(), 0);
    let layers = report.to_json()["layerSeconds"].clone();
    let layers = layers.as_array().expect("timed");
    assert_eq!(layers.len(), g.dims.y as usize);
    for (y, layer) in layers.iter().enumerate() {
        assert_eq!(layer["layer"].as_u64(), Some(y as u64));
        assert_eq!(layer["cells"].as_u64(), Some(per_layer as u64));
        assert_eq!(layer["mean"].as_f64(), Some((y + 1) as f64 / 1000.0));
        let sum = (per_layer * (y + 1)) as f64 / 1000.0;
        assert!((layer["seconds"].as_f64().unwrap() - sum).abs() < 1e-6);
        let bottom = g.origin.y + y as f32 * g.cell;
        assert!((layer["minY"].as_f64().unwrap() - bottom as f64).abs() < 0.01);
    }
}

#[test]
fn bake_occlusion_writes_its_report() {
    let path = scene_path("occlusion");
    let mut scene = corridors();
    let baked = bake_occlusion_only(&mut scene, Some(&path), &mut BakeLog::off()).unwrap();
    let report = read(&path, Kind::Occlusion).expect("the bake wrote its report");
    assert_common(&report, Kind::Occlusion);
    assert_eq!(report["cells"].as_u64(), Some(baked.occlusion.cells as u64));
    let size = std::fs::metadata(format!("{path}.occlusion"))
        .unwrap()
        .len();
    assert_eq!(report["outputBytes"].as_u64(), Some(size));
    assert_eq!(report["stages"][0]["name"], "bake");
    let slowest = report["slowestCells"]
        .as_array()
        .expect("the bake timed its cells");
    assert!(!slowest.is_empty() && slowest.len() <= TOP);
    let secs: Vec<f64> = slowest
        .iter()
        .map(|c| c["seconds"].as_f64().unwrap())
        .collect();
    assert!(
        secs.windows(2).all(|w| w[0] >= w[1]),
        "slowest first: {secs:?}"
    );
    assert_eq!(report["cellSeconds"]["max"].as_f64(), Some(secs[0]));
}
