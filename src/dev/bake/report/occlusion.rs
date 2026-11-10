//! The occlusion bake's report (#1980): the grid, the objects baked with their
//! triangles (the biggest occluders first), and how well the finished PVS culls:
//! the mean and most objects a cell sees, the share culled, the objects every
//! cell sees, and the time each cell took with the slowest cells (#1987). The
//! times come from the cell clocks the bake was lent (`dev::bake::occlusion::
//! CellClocks`), the rest from the bake's report and its PVS, outside the bake.
//! Being read outside, `neverCulled` is a count with no names: the objects too large or not finite
//! to sample are only known inside the bake. Every cell sees them, so they are among
//! `alwaysVisibleNames`, not told apart from the objects every cell sees anyway.

use std::time::Duration;

use serde_json::{json, Value};

use super::{entity_name, point, Kind, Report, Stages, TOP};
use crate::scene::lighting::occlusion::{
    OcclusionBakeReport, OcclusionData, CELL_SIZE, MAX_CELL, OCCLUSION_SEED,
};
use crate::scene::Scene;

/// The report of a bake of `scene` that reported `r`, baked `data`, took `cells`
/// per cell (by cell index; empty when it was not timed) and wrote `output_bytes`.
pub fn report(
    scene: &Scene,
    (r, data, cells): (&OcclusionBakeReport, Option<&OcclusionData>, &[Duration]),
    stages: &Stages,
    output_bytes: u64,
) -> Report {
    let settings = json!({
        "seed": OCCLUSION_SEED,
        "cellSize": CELL_SIZE,
        "maxCell": MAX_CELL,
    });
    let fingerprint = data.map_or(0, fingerprint);
    let (triangles, biggest) = data.map_or((0, json!([])), |d| occluder_triangles(scene, d));
    Report::new(Kind::Occlusion, stages, settings, fingerprint, output_bytes).with(json!({
        "cells": r.cells,
        "cellSize": r.cell_size,
        "tooLarge": r.too_large,
        "rows": r.rows,
        "dataBytes": r.bytes,
        "objects": r.objects,
        "occluders": r.occluders,
        "neverCulled": r.never_culled,
        "occluderTriangles": triangles,
        "biggestOccluders": biggest,
        "visibility": data.map_or(Value::Null, |d| visibility(scene, d)),
        "cellSeconds": data.map_or(Value::Null, |d| cell_seconds(d, cells)),
        "slowestCells": data.map_or(Value::Null, |d| slowest_cells(d, cells)),
    }))
}

/// Write the report of an occlusion bake now on `scene` (#1980): it reported `r`,
/// took `ms` and `cells` per cell, and writing its file beside `path` took `saved`
/// (`None` when the file waits for a scene save, so nothing is written yet).
pub fn keep(
    scene: &Scene,
    path: &str,
    (r, ms, cells): (&OcclusionBakeReport, u64, &[Duration]),
    saved: Option<std::time::Duration>,
) {
    let mut stages = Stages::start();
    stages.push("bake", std::time::Duration::from_millis(ms));
    let mut written = 0;
    if let Some(took) = saved {
        stages.push("write", took);
        written = super::bytes_under(&crate::scene::lighting::occlusion::occlusion_path(path));
    }
    let data = scene.occlusion.as_deref();
    let report = report(scene, (r, data, cells), &stages, written);
    super::keep(path, report);
}

/// What the PVS was baked against: every object's entity and fingerprint.
fn fingerprint(data: &OcclusionData) -> u64 {
    let mut hash = crate::core::hash::Content::default();
    for o in &data.objects {
        hash.write(&o.entity.to_le_bytes());
        hash.write(&o.fingerprint.to_le_bytes());
    }
    hash.finish()
}

/// A baked object's drawn triangles in the scene as it is.
fn triangles(scene: &Scene, entity: u32) -> usize {
    scene.world.mesh(entity).map_or(0, |m| m.indices.len() / 3)
}

/// The occluders' triangles added up, and the [`TOP`] biggest (ties to the lower id).
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

/// How many objects each cell sees: the mean and the most, the share of the
/// objects culled on average, and the objects every cell sees (at most [`TOP`]
/// named; the count holds them all).
pub(super) fn visibility(scene: &Scene, data: &OcclusionData) -> Value {
    let (rows, cells) = data.parts();
    let objects = data.objects.len();
    let words = objects.div_ceil(64);
    if words == 0 || cells.is_empty() {
        return Value::Null;
    }
    // How many cells use each row: one pass over the rows then counts every cell.
    let mut uses = vec![0u64; rows.len() / words];
    for &row in cells {
        uses[row as usize] += 1;
    }
    let (mut seen_by, mut sum, mut most) = (vec![0u64; objects], 0u64, 0u32);
    for (row, bits) in rows.chunks_exact(words).enumerate() {
        let n: u32 = bits.iter().map(|w| w.count_ones()).sum();
        if uses[row] == 0 {
            continue;
        }
        (sum, most) = (sum + n as u64 * uses[row], most.max(n));
        // Only the set bits: a cell sees a fraction of a big level.
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
    let mean = sum as f64 / cells.len() as f64;
    let every = (data.objects.iter().zip(&seen_by)).filter(|(_, &n)| n == cells.len() as u64);
    let every: Vec<u32> = every.map(|(o, _)| o.entity).collect();
    let named = every.iter().take(TOP).map(|&id| entity_name(scene, id));
    json!({
        "meanVisible": (mean * 100.0).round() / 100.0,
        "maxVisible": most,
        "culledFraction": ((1.0 - mean / objects as f64) * 1000.0).round() / 1000.0,
        "alwaysVisible": every.len(),
        "alwaysVisibleNames": named.collect::<Vec<_>>(),
    })
}

/// A cell's time in seconds, to the microsecond: most cells take less than the
/// millisecond the stages round to.
fn cell_secs(d: Duration) -> f64 {
    (d.as_secs_f64() * 1e6).round() / 1e6
}

/// The times of `data`'s cells, `None` when they were not timed (or not for this
/// grid).
fn timed<'a>(data: &OcclusionData, cells: &'a [Duration]) -> Option<&'a [Duration]> {
    (!cells.is_empty() && cells.len() == data.grid.cell_count()).then_some(cells)
}

/// The mean and the most time a cell took.
fn cell_seconds(data: &OcclusionData, cells: &[Duration]) -> Value {
    let Some(cells) = timed(data, cells) else {
        return Value::Null;
    };
    let total: Duration = cells.iter().sum();
    let max = cells.iter().max().copied().unwrap_or_default();
    json!({
        "mean": cell_secs(total / cells.len() as u32),
        "max": cell_secs(max),
    })
}

/// The [`TOP`] slowest cells, slowest first (ties to the lower index): each its
/// index, centre, time and how many objects its PVS row sees.
fn slowest_cells(data: &OcclusionData, cells: &[Duration]) -> Value {
    let Some(cells) = timed(data, cells) else {
        return Value::Null;
    };
    let mut order: Vec<usize> = (0..cells.len()).collect();
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
            "seconds": cell_secs(cells[cell]),
            "visible": visible(cell),
        })
    };
    Value::Array(order.iter().take(TOP).map(entry).collect())
}
