use std::time::Duration;

use serde_json::{json, Value};

use super::{entity_name, point, Kind, Report, Stages, TOP};
use rusty_lighting::occlusion::{OcclusionBakeReport, OCCLUSION_SEED, OCCLUSION_VERSION};
use rusty_scene::lighting::occlusion::{OcclusionData, CELL_SIZE, MAX_CELL};
use rusty_scene::Scene;

pub type CellTime = Option<Duration>;

pub type Measured<'a> = (&'a [CellTime], Option<&'a [bool]>);

pub fn report(
    scene: &Scene,
    (r, data, (cells, standable)): (&OcclusionBakeReport, Option<&OcclusionData>, Measured),
    stages: &Stages,
    output_bytes: u64,
) -> Report {
    let settings = json!({
        "version": OCCLUSION_VERSION,
        "seed": OCCLUSION_SEED,
        "cellSize": CELL_SIZE,
        "maxCell": MAX_CELL,
        "playHeight": r.play_height,
    });
    let fingerprint = data.map_or(0, fingerprint);
    let (triangles, biggest) = data.map_or((0, json!([])), |d| occluder_triangles(scene, d));
    Report::new(Kind::Occlusion, stages, settings, fingerprint, output_bytes).with(json!({
        "cells": r.cells,
        "bakedCells": r.baked_cells,
        "unbakedCells": r.cells - r.baked_cells.min(r.cells),
        "floorSource": r.floor.name(),
        "navmeshMissed": r.navmesh_missed,
        "floorCoverage": r.floor_coverage.map(|c| (c * 1000.0).round() / 1000.0),
        "cellSize": r.cell_size,
        "tooLarge": r.too_large,
        "rows": r.rows,
        "dataBytes": r.bytes,
        "objects": r.objects,
        "occluders": r.occluders,
        "neverCulled": r.never_culled,
        "occluderTriangles": triangles,
        "biggestOccluders": biggest,
        "visibility": data.map_or(Value::Null, |d| visibility(scene, d, standable)),
        "cellSeconds": data.map_or(Value::Null, |d| cell_seconds(d, cells)),
        "layerSeconds": data.map_or(Value::Null, |d| layer_seconds(d, cells)),
        "slowestCells": data.map_or(Value::Null, |d| slowest_cells(d, cells)),
    }))
}

pub const SKIPPED: &str = "skipped: unchanged";

pub fn keep(
    scene: &Scene,
    path: &str,
    (r, ms, cells): (&OcclusionBakeReport, u64, Measured),
    (saved, kept): (Option<std::time::Duration>, bool),
) {
    let mut stages = Stages::start();
    let name = if kept { SKIPPED } else { "bake" };
    stages.push(name, std::time::Duration::from_millis(ms));
    let mut written = 0;
    if let Some(took) = saved {
        stages.push("write", took);
        written = super::bytes_under(&rusty_scene::lighting::occlusion::occlusion_path(path));
    }
    let data = scene.occlusion.as_deref();
    let report = report(scene, (r, data, cells), &stages, written);
    super::keep(path, report);
}

fn fingerprint(data: &OcclusionData) -> u64 {
    let mut hash = rusty_core::hash::Content::default();
    for o in &data.objects {
        hash.write(&o.entity.to_le_bytes());
        hash.write(&o.fingerprint.to_le_bytes());
    }
    hash.finish()
}

fn triangles(scene: &Scene, entity: u32) -> usize {
    scene.world.mesh(entity).map_or(0, |m| m.indices.len() / 3)
}

fn occluder_triangles(scene: &Scene, data: &OcclusionData) -> (usize, Value) {
    let occluders = data.objects.iter().filter(|o| o.occluder);
    let mut weighed: Vec<(u32, usize)> = occluders
        .map(|o| (o.entity, triangles(scene, o.entity)))
        .collect();
    let total = weighed.iter().map(|w| w.1).sum();
    weighed.sort_by_key(|&(id, n)| (std::cmp::Reverse(n), id));
    let top = weighed
        .iter()
        .take(TOP)
        .map(|&(id, n)| json!({ "entity": id, "name": entity_name(scene, id), "triangles": n }));
    (total, Value::Array(top.collect()))
}

pub(super) fn visibility(scene: &Scene, data: &OcclusionData, standable: Option<&[bool]>) -> Value {
    let (rows, cells) = data.parts();
    let objects = data.objects.len();
    let words = objects.div_ceil(64);
    let counted = |c: &usize| standable.is_none_or(|s| s.get(*c).copied().unwrap_or(false));
    let counted: Vec<u32> = (0..cells.len()).filter(counted).map(|c| cells[c]).collect();
    if words == 0 || counted.is_empty() {
        return Value::Null;
    }
    let mut uses = vec![0u64; rows.len() / words];
    for &row in &counted {
        uses[row as usize] += 1;
    }
    let (mut seen_by, mut sum, mut most) = (vec![0u64; objects], 0u64, 0u32);
    for (row, bits) in rows.chunks_exact(words).enumerate() {
        let n: u32 = bits.iter().map(|w| w.count_ones()).sum();
        if uses[row] == 0 {
            continue;
        }
        (sum, most) = (sum + n as u64 * uses[row], most.max(n));
        for (w, &word) in bits.iter().enumerate() {
            let mut word = word;
            while word != 0 {
                let i = w * 64 + word.trailing_zeros() as usize;
                if let Some(seen) = seen_by.get_mut(i) {
                    *seen += uses[row];
                }
                word &= word - 1;
            }
        }
    }
    let total = counted.len() as u64;
    let mean = sum as f64 / total as f64;
    let every = (data.objects.iter().zip(&seen_by)).filter(|(_, &n)| n == total);
    let every: Vec<u32> = every.map(|(o, _)| o.entity).collect();
    let named = every.iter().take(TOP).map(|&id| entity_name(scene, id));
    json!({
        "cells": total,
        "meanVisible": (mean * 100.0).round() / 100.0,
        "maxVisible": most,
        "culledFraction": ((1.0 - mean / objects as f64) * 1000.0).round() / 1000.0,
        "alwaysVisible": every.len(),
        "alwaysVisibleNames": named.collect::<Vec<_>>(),
    })
}

/// To the microsecond: most cells take under the millisecond [`super::secs`] rounds to.
fn cell_secs(d: Duration) -> f64 {
    (d.as_secs_f64() * 1e6).round() / 1e6
}

fn timed<'a>(data: &OcclusionData, cells: &'a [CellTime]) -> Option<&'a [CellTime]> {
    (!cells.is_empty() && cells.len() == data.grid.cell_count()).then_some(cells)
}

fn summed(cells: &[CellTime]) -> (Duration, Duration, Duration, usize) {
    let times = cells.iter().flatten();
    let (total, n): (Duration, usize) = (times.clone().sum(), times.clone().count());
    let max = times.max().copied().unwrap_or_default();
    (total, total / n.max(1) as u32, max, n)
}

fn cell_seconds(data: &OcclusionData, cells: &[CellTime]) -> Value {
    let Some(cells) = timed(data, cells) else {
        return Value::Null;
    };
    let (_, mean, max, _) = summed(cells);
    json!({
        "mean": cell_secs(mean),
        "max": cell_secs(max),
    })
}

fn layer_seconds(data: &OcclusionData, cells: &[CellTime]) -> Value {
    let Some(cells) = timed(data, cells) else {
        return Value::Null;
    };
    let grid = &data.grid;
    let per_layer = (grid.dims.x * grid.dims.z) as usize;
    let layer = |(y, times): (usize, &[CellTime])| {
        let (total, mean, _, sampled) = summed(times);
        let bottom = grid.origin.y + y as f32 * grid.cell;
        json!({
            "layer": y,
            "minY": (bottom * 100.0).round() / 100.0,
            "maxY": ((bottom + grid.cell) * 100.0).round() / 100.0,
            "cells": sampled,
            "seconds": cell_secs(total),
            "mean": cell_secs(mean),
        })
    };
    let layers = cells.chunks(per_layer.max(1)).enumerate().map(layer);
    Value::Array(layers.collect())
}

fn slowest_cells(data: &OcclusionData, cells: &[CellTime]) -> Value {
    let Some(cells) = timed(data, cells) else {
        return Value::Null;
    };
    let mut order: Vec<usize> = (0..cells.len()).filter(|&i| cells[i].is_some()).collect();
    order.sort_by_key(|&i| (std::cmp::Reverse(cells[i]), i));
    let (rows, rows_of) = data.parts();
    let words = data.objects.len().div_ceil(64);
    let visible = |cell: usize| -> u32 {
        let row = rows_of[cell] as usize * words;
        rows[row..row + words].iter().map(|w| w.count_ones()).sum()
    };
    let entry = |&cell: &usize| {
        let (min, max) = data.grid.bounds(cell);
        json!({
            "cell": cell,
            "centre": point((min + max) * 0.5),
            "seconds": cell_secs(cells[cell].unwrap_or_default()),
            "visible": visible(cell),
        })
    };
    Value::Array(order.iter().take(TOP).map(entry).collect())
}
