use std::time::Duration;

use crate::bake::report::{occlusion, point, Stages, TOP};
use rusty_lighting::occlusion::bake_occlusion;
use rusty_lighting::occlusion::test_level::corridors;

mod kept;
mod reach;

#[test]
fn the_corridors_cull_a_share_of_their_objects() {
    let scene = corridors();
    let (data, r) = bake_occlusion(&scene, 2);
    let data = data.expect("the corridors occlude");
    let report =
        occlusion::report(&scene, (&r, Some(&data), (&[], None)), &Stages::start(), 0).to_json();
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
    let mut cells = vec![Some(Duration::from_millis(1)); r.cells];
    (cells[3], cells[1]) = (
        Some(Duration::from_millis(9)),
        Some(Duration::from_millis(5)),
    );
    let report = occlusion::report(
        &scene,
        (&r, Some(&data), (&cells, None)),
        &Stages::start(),
        0,
    );
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
    let other = occlusion::report(
        &scene,
        (&r, Some(&data), (&cells[1..], None)),
        &Stages::start(),
        0,
    );
    assert!(other.to_json()["slowestCells"].is_null());
}

#[test]
fn each_grid_layer_sums_its_cells_times() {
    let scene = corridors();
    let (data, r) = bake_occlusion(&scene, 2);
    let data = data.expect("the corridors occlude");
    let g = data.grid;
    let per_layer = (g.dims.x * g.dims.z) as usize;
    let cells: Vec<_> = (0..r.cells)
        .map(|i| Some(Duration::from_millis((i / per_layer) as u64 + 1)))
        .collect();
    let report = occlusion::report(
        &scene,
        (&r, Some(&data), (&cells, None)),
        &Stages::start(),
        0,
    );
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
